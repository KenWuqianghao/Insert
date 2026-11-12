use std::collections::HashSet;
use std::io;
use std::path::PathBuf;

use time::OffsetDateTime;

use crate::capture::Capture;
use crate::mime::MimeMap;
use crate::model::{Clip, ClipId, ClipKind, Pinboard, PinboardColor, PinboardId, SourceApp};
use crate::prefs::HistoryLimit;
use crate::storage::Storage;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Scope {
    History,
    Pinboard(PinboardId),
}

/// What a library method changed. Refresh the views that show it. Save the index (with a short
/// debounce) when `needs_save` is true.
#[must_use]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Change {
    pub clips: bool,
    pub pinboards: bool,
    pub removed: Vec<ClipId>,
}

impl Change {
    pub fn needs_save(&self) -> bool {
        self.clips || self.pinboards
    }

    pub fn merge(mut self, other: Change) -> Change {
        self.clips |= other.clips;
        self.pinboards |= other.pinboards;
        self.removed.extend(other.removed);
        self
    }

    fn pinboards() -> Change {
        Change {
            pinboards: true,
            ..Change::default()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmptyState {
    NoMatches,
    NoClips,
    EmptyPinboard,
}

impl EmptyState {
    pub fn icon_name(self) -> &'static str {
        match self {
            Self::NoMatches => "edit-find-symbolic",
            Self::NoClips => "edit-paste-symbolic",
            Self::EmptyPinboard => "view-pin-symbolic",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::NoMatches => "No Matches",
            Self::NoClips => "No Clips",
            Self::EmptyPinboard => "Empty Pinboard",
        }
    }

    pub fn detail(self) -> &'static str {
        match self {
            Self::NoMatches => "Change the search text or the type filter.",
            Self::NoClips => "Copy something. It shows here.",
            Self::EmptyPinboard => "Right-click a clip and select Pin to.",
        }
    }
}

pub fn empty_state(scope: Scope, kind: Option<ClipKind>, query: &str) -> EmptyState {
    if !query.trim().is_empty() || kind.is_some() {
        return EmptyState::NoMatches;
    }
    match scope {
        Scope::History => EmptyState::NoClips,
        Scope::Pinboard(_) => EmptyState::EmptyPinboard,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NameTarget {
    Clip(ClipId),
    Pinboard(PinboardId),
    NewPinboard {
        pinning: HashSet<ClipId>,
    },
}

impl NameTarget {
    pub fn prompt_title(&self) -> &'static str {
        match self {
            Self::Clip(_) => "Rename Clip",
            Self::Pinboard(_) => "Rename Pinboard",
            Self::NewPinboard { .. } => "New Pinboard",
        }
    }
}

#[must_use]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NameCommit {
    pub change: Change,
    pub switch_to: Option<Scope>,
}

pub struct Opened {
    pub library: Library,
    pub corrupt_backup: Option<PathBuf>,
}

/// All clips (newest first by "last captured or last used") and pinboards, with their files.
/// Methods that change files do the file work at once. Index saves are left to the caller.
#[derive(Debug)]
pub struct Library {
    clips: Vec<Clip>,
    pinboards: Vec<Pinboard>,
    history_limit: HistoryLimit,
    storage: Storage,
}

impl Library {
    /// Loads the index, drops clips whose payload is missing, and deletes orphan files. While a
    /// corrupt index backup exists, no file is deleted, so the user can still recover the clips.
    pub fn open(storage: Storage, history_limit: HistoryLimit) -> Opened {
        let loaded = storage.load_index();
        let clips: Vec<Clip> = loaded
            .index
            .clips
            .into_iter()
            .filter(|clip| storage.has_payload(clip.id))
            .collect();
        if !storage.has_corrupt_index_backup() {
            storage.remove_orphans(&clips.iter().map(|clip| clip.id).collect());
        }
        let library = Library {
            clips,
            pinboards: loaded.index.pinboards,
            history_limit,
            storage,
        };
        Opened {
            library,
            corrupt_backup: loaded.corrupt_backup,
        }
    }

    pub fn clips(&self) -> &[Clip] {
        &self.clips
    }

    pub fn pinboards(&self) -> &[Pinboard] {
        &self.pinboards
    }

    pub fn clip(&self, id: ClipId) -> Option<&Clip> {
        self.clips.iter().find(|clip| clip.id == id)
    }

    pub fn pinboard(&self, id: PinboardId) -> Option<&Pinboard> {
        self.pinboards.iter().find(|pinboard| pinboard.id == id)
    }

    pub fn history_limit(&self) -> HistoryLimit {
        self.history_limit
    }

    pub fn payload(&self, id: ClipId) -> io::Result<MimeMap> {
        self.storage.load_payload(id)
    }

    pub fn thumbnail_path(&self, id: ClipId) -> PathBuf {
        self.storage.thumbnail_path(id)
    }

    pub fn save(&self) -> io::Result<()> {
        self.storage.save_index(&self.clips, &self.pinboards)
    }

    pub fn set_history_limit(&mut self, limit: HistoryLimit) -> Change {
        self.history_limit = limit;
        self.update(|clips| trim(clips, limit))
    }

    /// A clip with the same content hash takes the new payload, which can differ in volatile
    /// entries. It moves to the front with the new time and source, and keeps its title and
    /// pinboard. Else the payload is written and a new clip is inserted.
    pub fn insert(
        &mut self,
        capture: Capture,
        source: Option<SourceApp>,
        now: OffsetDateTime,
    ) -> io::Result<Change> {
        let now = whole_seconds(now);
        if let Some(existing) = self
            .clips
            .iter()
            .find(|clip| clip.content_hash == capture.content_hash)
        {
            let id = existing.id;
            self.storage
                .write_clip(id, &capture.payload, capture.thumbnail_png.as_deref())?;
            return Ok(self.update(|clips| {
                move_to_front(clips, id, now, |clip| {
                    clip.byte_count = capture.byte_count;
                    if source.is_some() {
                        clip.source = source;
                    }
                })
            }));
        }

        let clip = Clip {
            id: ClipId::new(),
            created_at: now,
            source,
            content: capture.content,
            custom_title: None,
            pinboard_id: None,
            content_hash: capture.content_hash,
            byte_count: capture.byte_count,
        };
        self.storage
            .write_clip(clip.id, &capture.payload, capture.thumbnail_png.as_deref())?;
        let limit = self.history_limit;
        Ok(self.update(|clips| {
            clips.insert(0, clip);
            trim(clips, limit);
        }))
    }

    pub fn move_to_front(&mut self, id: ClipId, now: OffsetDateTime) -> Change {
        let now = whole_seconds(now);
        self.update(|clips| move_to_front(clips, id, now, |_| {}))
    }

    /// Pinned clips are deleted too.
    pub fn delete(&mut self, ids: &HashSet<ClipId>) -> Change {
        self.update(|clips| clips.retain(|clip| !ids.contains(&clip.id)))
    }

    pub fn clear_history(&mut self) -> Change {
        self.update(|clips| clips.retain(|clip| clip.pinboard_id.is_some()))
    }

    /// Pins (`Some`), moves, or unpins (`None`) the clips. Then the history limit applies.
    pub fn set_pinboard(&mut self, pinboard: Option<PinboardId>, ids: &HashSet<ClipId>) -> Change {
        let limit = self.history_limit;
        self.update(|clips| {
            for clip in clips.iter_mut().filter(|clip| ids.contains(&clip.id)) {
                clip.pinboard_id = pinboard;
            }
            trim(clips, limit);
        })
    }

    pub fn rename(&mut self, id: ClipId, title: Option<String>) -> Change {
        self.update(|clips| {
            if let Some(clip) = clips.iter_mut().find(|clip| clip.id == id) {
                clip.custom_title = title;
            }
        })
    }

    /// The new pinboard gets the first color that no pinboard uses. It goes at the end.
    pub fn add_pinboard(&mut self, name: String) -> (PinboardId, Change) {
        let used: HashSet<PinboardColor> = self
            .pinboards
            .iter()
            .map(|pinboard| pinboard.color)
            .collect();
        let color = PinboardColor::ALL
            .into_iter()
            .find(|color| !used.contains(color))
            .unwrap_or(PinboardColor::ALL[self.pinboards.len() % PinboardColor::ALL.len()]);
        let id = PinboardId::new();
        self.pinboards.push(Pinboard { id, name, color });
        (id, Change::pinboards())
    }

    pub fn rename_pinboard(&mut self, id: PinboardId, name: String) -> Change {
        self.update_pinboard(id, |pinboard| pinboard.name = name)
    }

    pub fn set_pinboard_color(&mut self, id: PinboardId, color: PinboardColor) -> Change {
        self.update_pinboard(id, |pinboard| pinboard.color = color)
    }

    /// Its clips go back to the history. Then the history limit applies to them.
    pub fn delete_pinboard(&mut self, id: PinboardId) -> Change {
        let before = self.pinboards.len();
        self.pinboards.retain(|pinboard| pinboard.id != id);
        if self.pinboards.len() == before {
            return Change::default();
        }
        let limit = self.history_limit;
        Change::pinboards().merge(self.update(|clips| {
            for clip in clips.iter_mut().filter(|clip| clip.pinboard_id == Some(id)) {
                clip.pinboard_id = None;
            }
            trim(clips, limit);
        }))
    }

    pub fn current_name(&self, target: &NameTarget) -> String {
        match target {
            NameTarget::Clip(id) => self.clip(*id).and_then(|clip| clip.custom_title.clone()),
            NameTarget::Pinboard(id) => self.pinboard(*id).map(|pinboard| pinboard.name.clone()),
            NameTarget::NewPinboard { .. } => None,
        }
        .unwrap_or_default()
    }

    /// `text` is trimmed first.
    pub fn commit_name(&mut self, target: &NameTarget, text: &str) -> NameCommit {
        let name = text.trim();
        match target {
            NameTarget::Clip(id) => {
                let title = Some(name.to_owned()).filter(|name| !name.is_empty());
                NameCommit {
                    change: self.rename(*id, title),
                    switch_to: None,
                }
            }
            NameTarget::Pinboard(_) | NameTarget::NewPinboard { .. } if name.is_empty() => {
                NameCommit::default()
            }
            NameTarget::Pinboard(id) => NameCommit {
                change: self.rename_pinboard(*id, name.to_owned()),
                switch_to: None,
            },
            NameTarget::NewPinboard { pinning } => {
                let (id, change) = self.add_pinboard(name.to_owned());
                if pinning.is_empty() {
                    NameCommit {
                        change,
                        switch_to: Some(Scope::Pinboard(id)),
                    }
                } else {
                    NameCommit {
                        change: change.merge(self.set_pinboard(Some(id), pinning)),
                        switch_to: None,
                    }
                }
            }
        }
    }

    /// Clips in library order, filtered by scope, then kind, then the trimmed query. The history
    /// scope shows every clip, pinned ones included.
    pub fn visible(&self, scope: Scope, kind: Option<ClipKind>, query: &str) -> Vec<&Clip> {
        let query = query.trim();
        self.clips
            .iter()
            .filter(|clip| match scope {
                Scope::History => true,
                Scope::Pinboard(id) => clip.pinboard_id == Some(id),
            })
            .filter(|clip| kind.is_none_or(|kind| clip.kind() == kind))
            .filter(|clip| query.is_empty() || clip.matches(query))
            .collect()
    }

    pub fn scopes(&self) -> Vec<Scope> {
        std::iter::once(Scope::History)
            .chain(
                self.pinboards
                    .iter()
                    .map(|pinboard| Scope::Pinboard(pinboard.id)),
            )
            .collect()
    }

    /// The scope `offset` tabs away, wrapping around. An unknown scope counts as history.
    pub fn step_scope(&self, scope: Scope, offset: isize) -> Scope {
        let scopes = self.scopes();
        let current = scopes.iter().position(|s| *s == scope).unwrap_or(0) as isize;
        scopes[(current + offset).rem_euclid(scopes.len() as isize) as usize]
    }

    /// History when `scope` names a pinboard that no longer exists.
    pub fn resolve_scope(&self, scope: Scope) -> Scope {
        match scope {
            Scope::Pinboard(id) if self.pinboard(id).is_none() => Scope::History,
            _ => scope,
        }
    }

    fn update(&mut self, change: impl FnOnce(&mut Vec<Clip>)) -> Change {
        let mut next = self.clips.clone();
        change(&mut next);
        if next == self.clips {
            return Change::default();
        }
        let kept: HashSet<ClipId> = next.iter().map(|clip| clip.id).collect();
        let removed: Vec<ClipId> = self
            .clips
            .iter()
            .map(|clip| clip.id)
            .filter(|id| !kept.contains(id))
            .collect();
        for id in &removed {
            self.storage.remove_files(*id);
        }
        self.clips = next;
        Change {
            clips: true,
            pinboards: false,
            removed,
        }
    }

    fn update_pinboard(&mut self, id: PinboardId, change: impl FnOnce(&mut Pinboard)) -> Change {
        match self.pinboards.iter_mut().find(|pinboard| pinboard.id == id) {
            Some(pinboard) => {
                change(pinboard);
                Change::pinboards()
            }
            None => Change::default(),
        }
    }
}

fn whole_seconds(time: OffsetDateTime) -> OffsetDateTime {
    time.replace_nanosecond(0).expect("0 is a valid nanosecond")
}

fn move_to_front(
    clips: &mut Vec<Clip>,
    id: ClipId,
    now: OffsetDateTime,
    adjust: impl FnOnce(&mut Clip),
) {
    if let Some(index) = clips.iter().position(|clip| clip.id == id) {
        let mut clip = clips.remove(index);
        clip.created_at = now;
        adjust(&mut clip);
        clips.insert(0, clip);
    }
}

fn trim(clips: &mut Vec<Clip>, limit: HistoryLimit) {
    let mut unpinned = 0;
    clips.retain(|clip| {
        if clip.pinboard_id.is_some() {
            return true;
        }
        unpinned += 1;
        unpinned <= limit.count()
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::classify;
    use crate::mime::UTF8_TEXT;
    use time::Duration;

    const WEB_CUSTOM_DATA: &str = "chromium/x-web-custom-data";

    struct Fixture {
        _dir: tempfile::TempDir,
        storage: Storage,
        library: Library,
    }

    fn fixture(limit: HistoryLimit) -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let storage = Storage::new(dir.path().to_owned());
        let library = Library::open(storage.clone(), limit).library;
        Fixture {
            _dir: dir,
            storage,
            library,
        }
    }

    fn t(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_800_000_000 + seconds).unwrap()
    }

    fn text(text: &str) -> Capture {
        classify(MimeMap(vec![(UTF8_TEXT.into(), text.as_bytes().to_vec())])).unwrap()
    }

    fn app(name: &str) -> Option<SourceApp> {
        Some(SourceApp {
            id: name.to_lowercase(),
            name: name.into(),
        })
    }

    fn insert(library: &mut Library, value: &str) -> ClipId {
        let _ = library.insert(text(value), None, t(0)).unwrap();
        library.clips()[0].id
    }

    fn ids(clips: &[&Clip]) -> Vec<ClipId> {
        clips.iter().map(|clip| clip.id).collect()
    }

    #[test]
    fn insert_writes_files_and_puts_the_clip_first() {
        let Fixture {
            storage,
            mut library,
            ..
        } = fixture(HistoryLimit::Clips100);
        let first = insert(&mut library, "a");
        let change = library.insert(text("b"), app("Kate"), t(5)).unwrap();
        assert!(change.clips && change.needs_save());
        let second = library.clips()[0].id;
        assert_eq!(
            library.clips().iter().map(|c| c.id).collect::<Vec<_>>(),
            [second, first]
        );
        assert!(storage.has_payload(second));
        assert_eq!(
            library.payload(second).unwrap().get(UTF8_TEXT),
            Some(b"b".as_slice())
        );
    }

    #[test]
    fn duplicate_moves_to_front_with_new_time_and_source_and_keeps_title_and_pinboard() {
        let Fixture { mut library, .. } = fixture(HistoryLimit::Clips100);
        let original = insert(&mut library, "same");
        let (board, _) = library.add_pinboard("Work".into());
        let _ = library.set_pinboard(Some(board), &HashSet::from([original]));
        let _ = library.rename(original, Some("Title".into()));
        insert(&mut library, "other");

        let _ = library.insert(text("same"), app("Firefox"), t(90)).unwrap();
        assert_eq!(library.clips().len(), 2);
        let clip = &library.clips()[0];
        assert_eq!(clip.id, original);
        assert_eq!(clip.created_at, t(90));
        assert_eq!(clip.source, app("Firefox"));
        assert_eq!(clip.custom_title.as_deref(), Some("Title"));
        assert_eq!(clip.pinboard_id, Some(board));

        let _ = library.insert(text("same"), None, t(95)).unwrap();
        assert_eq!(library.clips()[0].source, app("Firefox"));
    }

    #[test]
    fn duplicate_takes_the_volatile_entries_of_the_new_capture() {
        let Fixture { mut library, .. } = fixture(HistoryLimit::Clips100);
        let capture = |custom: &[u8]| {
            classify(MimeMap(vec![
                (UTF8_TEXT.into(), b"same".to_vec()),
                (WEB_CUSTOM_DATA.into(), custom.to_vec()),
            ]))
            .unwrap()
        };
        let _ = library.insert(capture(b"old"), None, t(0)).unwrap();
        let id = library.clips()[0].id;

        let _ = library.insert(capture(b"newer"), None, t(5)).unwrap();
        assert_eq!(library.clips().len(), 1);
        assert_eq!(
            library.payload(id).unwrap().get(WEB_CUSTOM_DATA),
            Some(b"newer".as_slice())
        );
        assert_eq!(library.clips()[0].byte_count, 9);
    }

    #[test]
    fn pruning_keeps_pinned_clips_and_the_newest_unpinned() {
        let Fixture {
            storage,
            mut library,
            ..
        } = fixture(HistoryLimit::Clips100);
        let (board, _) = library.add_pinboard("Keep".into());
        let mut pinned = HashSet::new();
        for index in 0..10 {
            pinned.insert(insert(&mut library, &format!("pinned {index}")));
        }
        let _ = library.set_pinboard(Some(board), &pinned);
        let oldest = insert(&mut library, "unpinned 0");
        for index in 1..150 {
            insert(&mut library, &format!("unpinned {index}"));
        }
        assert_eq!(library.clips().len(), 110);
        assert!(pinned.iter().all(|id| library.clip(*id).is_some()));
        assert!(library.clip(oldest).is_none());
        assert!(!storage.has_payload(oldest));
    }

    #[test]
    fn unpin_and_pinboard_delete_prune_at_once() {
        let Fixture { mut library, .. } = fixture(HistoryLimit::Clips100);
        let (board, _) = library.add_pinboard("Board".into());
        let pinned: HashSet<ClipId> = (0..5)
            .map(|i| insert(&mut library, &format!("p{i}")))
            .collect();
        let _ = library.set_pinboard(Some(board), &pinned);
        for index in 0..100 {
            insert(&mut library, &format!("h{index}"));
        }
        assert_eq!(library.clips().len(), 105);

        let one: HashSet<ClipId> = pinned.iter().take(1).copied().collect();
        let change = library.set_pinboard(None, &one);
        assert_eq!(change.removed.len(), 1);
        assert_eq!(library.clips().len(), 104);

        let change = library.delete_pinboard(board);
        assert!(change.pinboards && change.clips);
        assert_eq!(change.removed.len(), 4);
        assert_eq!(library.clips().len(), 100);
        assert!(library
            .clips()
            .iter()
            .all(|clip| clip.pinboard_id.is_none()));
        assert_eq!(
            library.resolve_scope(Scope::Pinboard(board)),
            Scope::History
        );
    }

    #[test]
    fn lowering_the_history_limit_prunes() {
        let Fixture { mut library, .. } = fixture(HistoryLimit::Clips500);
        for index in 0..120 {
            insert(&mut library, &format!("{index}"));
        }
        let change = library.set_history_limit(HistoryLimit::Clips100);
        assert_eq!(change.removed.len(), 20);
        assert_eq!(library.clips().len(), 100);
    }

    #[test]
    fn delete_removes_pinned_clips_and_files() {
        let Fixture {
            storage,
            mut library,
            ..
        } = fixture(HistoryLimit::Clips100);
        let (board, _) = library.add_pinboard("B".into());
        let a = insert(&mut library, "a");
        let b = insert(&mut library, "b");
        let _ = library.set_pinboard(Some(board), &HashSet::from([a]));
        let change = library.delete(&HashSet::from([a, b]));
        assert_eq!(change.removed.len(), 2);
        assert!(library.clips().is_empty());
        assert!(!storage.has_payload(a));
        assert!(library.delete(&HashSet::from([a])) == Change::default());
    }

    #[test]
    fn clear_history_keeps_pinned_clips() {
        let Fixture { mut library, .. } = fixture(HistoryLimit::Clips100);
        let (board, _) = library.add_pinboard("B".into());
        let pinned = insert(&mut library, "pinned");
        let _ = library.set_pinboard(Some(board), &HashSet::from([pinned]));
        insert(&mut library, "loose");
        let _ = library.clear_history();
        assert_eq!(
            library.clips().iter().map(|c| c.id).collect::<Vec<_>>(),
            [pinned]
        );
    }

    #[test]
    fn move_to_front_sets_the_time() {
        let Fixture { mut library, .. } = fixture(HistoryLimit::Clips100);
        let old = insert(&mut library, "old");
        insert(&mut library, "new");
        let _ = library.move_to_front(old, t(30) + Duration::milliseconds(400));
        assert_eq!(library.clips()[0].id, old);
        assert_eq!(library.clips()[0].created_at, t(30));
    }

    #[test]
    fn new_pinboards_take_the_first_free_color_then_cycle() {
        let Fixture { mut library, .. } = fixture(HistoryLimit::Clips100);
        let colors: Vec<PinboardColor> = (0..10)
            .map(|index| {
                let (id, _) = library.add_pinboard(format!("{index}"));
                library.pinboard(id).unwrap().color
            })
            .collect();
        assert_eq!(colors[..8], PinboardColor::ALL);
        assert_eq!(colors[8], PinboardColor::ALL[0]);
        assert_eq!(colors[9], PinboardColor::ALL[1]);
        let (first, second) = (library.pinboards()[0].id, library.pinboards()[1].id);
        let _ = library.set_pinboard_color(first, PinboardColor::Blue);
        let _ = library.delete_pinboard(second);
        let (id, _) = library.add_pinboard("next".into());
        assert_eq!(library.pinboard(id).unwrap().color, PinboardColor::Orange);
    }

    #[test]
    fn name_commit_rules() {
        let Fixture { mut library, .. } = fixture(HistoryLimit::Clips100);
        let clip = insert(&mut library, "x");
        let _ = library.commit_name(&NameTarget::Clip(clip), "  Title \n");
        assert_eq!(
            library.clip(clip).unwrap().custom_title.as_deref(),
            Some("Title")
        );
        assert_eq!(library.current_name(&NameTarget::Clip(clip)), "Title");
        let _ = library.commit_name(&NameTarget::Clip(clip), "  ");
        assert_eq!(library.clip(clip).unwrap().custom_title, None);

        let empty = NameTarget::NewPinboard {
            pinning: HashSet::new(),
        };
        assert_eq!(library.commit_name(&empty, " "), NameCommit::default());
        assert!(library.pinboards().is_empty());

        let commit = library.commit_name(&empty, "Work");
        let board = library.pinboards()[0].id;
        assert_eq!(commit.switch_to, Some(Scope::Pinboard(board)));

        let _ = library.commit_name(&NameTarget::Pinboard(board), "");
        assert_eq!(library.pinboards()[0].name, "Work");
        let _ = library.commit_name(&NameTarget::Pinboard(board), "Home");
        assert_eq!(library.pinboards()[0].name, "Home");

        let pinning = NameTarget::NewPinboard {
            pinning: HashSet::from([clip]),
        };
        let commit = library.commit_name(&pinning, "Pins");
        assert_eq!(commit.switch_to, None);
        assert_eq!(
            library.clip(clip).unwrap().pinboard_id,
            Some(library.pinboards()[1].id)
        );
    }

    #[test]
    fn visible_filters_by_scope_kind_and_trimmed_query() {
        let Fixture { mut library, .. } = fixture(HistoryLimit::Clips100);
        let link = insert(&mut library, "https://example.com");
        let apple = insert(&mut library, "Apple pie");
        let (board, _) = library.add_pinboard("Food".into());
        let _ = library.set_pinboard(Some(board), &HashSet::from([apple]));

        assert_eq!(
            ids(&library.visible(Scope::History, None, "")),
            [apple, link]
        );
        assert_eq!(
            ids(&library.visible(Scope::Pinboard(board), None, "")),
            [apple]
        );
        assert_eq!(
            ids(&library.visible(Scope::History, Some(ClipKind::Link), "")),
            [link]
        );
        assert_eq!(
            ids(&library.visible(Scope::History, None, "  apple\n")),
            [apple]
        );
        assert_eq!(ids(&library.visible(Scope::History, None, "link")), [link]);
        assert!(library
            .visible(Scope::History, Some(ClipKind::Image), "")
            .is_empty());
    }

    #[test]
    fn empty_states_in_order() {
        let board = Scope::Pinboard(PinboardId::new());
        assert_eq!(
            empty_state(Scope::History, None, "x"),
            EmptyState::NoMatches
        );
        assert_eq!(
            empty_state(board, Some(ClipKind::Text), ""),
            EmptyState::NoMatches
        );
        assert_eq!(empty_state(Scope::History, None, "  "), EmptyState::NoClips);
        assert_eq!(empty_state(board, None, ""), EmptyState::EmptyPinboard);
    }

    #[test]
    fn scope_steps_wrap_around() {
        let Fixture { mut library, .. } = fixture(HistoryLimit::Clips100);
        let (a, _) = library.add_pinboard("A".into());
        let (b, _) = library.add_pinboard("B".into());
        assert_eq!(library.step_scope(Scope::History, 1), Scope::Pinboard(a));
        assert_eq!(library.step_scope(Scope::Pinboard(b), 1), Scope::History);
        assert_eq!(library.step_scope(Scope::History, -1), Scope::Pinboard(b));
    }

    #[test]
    fn reopen_restores_clips_and_drops_clips_without_payload() {
        let Fixture {
            _dir,
            storage,
            mut library,
        } = fixture(HistoryLimit::Clips100);
        let (board, _) = library.add_pinboard("Work".into());
        let kept = insert(&mut library, "kept");
        let lost = insert(&mut library, "lost");
        let _ = library.set_pinboard(Some(board), &HashSet::from([kept]));
        library.save().unwrap();
        std::fs::remove_dir_all(storage.payload_dir(lost)).unwrap();

        let reopened = Library::open(storage.clone(), HistoryLimit::Clips100);
        assert_eq!(reopened.corrupt_backup, None);
        let library = reopened.library;
        assert_eq!(
            library.clips().iter().map(|c| c.id).collect::<Vec<_>>(),
            [kept]
        );
        assert_eq!(library.clip(kept).unwrap().pinboard_id, Some(board));
        assert_eq!(library.pinboards()[0].name, "Work");
    }

    #[test]
    fn corrupt_index_opens_an_empty_library_and_keeps_the_files() {
        let Fixture {
            _dir,
            storage,
            mut library,
        } = fixture(HistoryLimit::Clips100);
        let id = insert(&mut library, "x");
        std::fs::write(storage.index_path(), b"\0garbage").unwrap();

        let reopened = Library::open(storage.clone(), HistoryLimit::Clips100);
        assert!(reopened.corrupt_backup.is_some_and(|path| path.exists()));
        assert!(reopened.library.clips().is_empty());
        assert!(storage.has_payload(id));
        reopened.library.save().unwrap();

        let second = Library::open(storage.clone(), HistoryLimit::Clips100);
        assert_eq!(second.corrupt_backup, None);
        assert!(storage.has_payload(id));
    }

    #[test]
    fn unreadable_index_is_moved_aside_and_keeps_the_files() {
        let Fixture {
            _dir,
            storage,
            mut library,
        } = fixture(HistoryLimit::Clips100);
        let id = insert(&mut library, "x");
        let _ = std::fs::remove_file(storage.index_path());
        std::fs::create_dir(storage.index_path()).unwrap();

        let reopened = Library::open(storage.clone(), HistoryLimit::Clips100);
        assert!(reopened.corrupt_backup.is_some_and(|path| path.is_dir()));
        assert!(reopened.library.clips().is_empty());
        assert!(!storage.index_path().exists());
        reopened.library.save().unwrap();

        let _ = Library::open(storage.clone(), HistoryLimit::Clips100);
        assert!(storage.has_payload(id));
    }
}
