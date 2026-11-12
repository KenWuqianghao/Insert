use std::collections::HashSet;

use insert_core::{ClipId, ClipKind, Library, NameTarget, Scope, Selection, TrayMode};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrayState {
    Hidden,
    Browsing,
    Previewing,
    Naming(NameTarget),
    Hiding { then: Option<Paste> },
}

/// Paste into the front app when the hide ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Paste;

#[derive(Debug)]
pub struct TrayModel {
    pub state: TrayState,
    pub scope: Scope,
    pub kind: Option<ClipKind>,
    pub query: String,
    pub selection: Selection,
    pub ctrl_held: bool,
    pub deleting: HashSet<ClipId>,
    pub menu_targets: HashSet<ClipId>,
    pub visible: Vec<ClipId>,
}

impl TrayModel {
    pub fn new() -> Self {
        Self {
            state: TrayState::Hidden,
            scope: Scope::History,
            kind: None,
            query: String::new(),
            selection: Selection::default(),
            ctrl_held: false,
            deleting: HashSet::new(),
            menu_targets: HashSet::new(),
            visible: Vec::new(),
        }
    }

    pub fn is_shown(&self) -> bool {
        matches!(
            self.state,
            TrayState::Browsing | TrayState::Previewing | TrayState::Naming(_)
        )
    }

    pub fn key_mode(&self) -> Option<TrayMode> {
        match self.state {
            TrayState::Browsing => Some(TrayMode::Browsing),
            TrayState::Previewing => Some(TrayMode::Previewing),
            TrayState::Naming(_) => Some(TrayMode::Naming),
            TrayState::Hidden | TrayState::Hiding { .. } => None,
        }
    }

    fn compute_visible(&self, library: &Library) -> Vec<ClipId> {
        library
            .visible(self.scope, self.kind, &self.query)
            .into_iter()
            .map(|clip| clip.id)
            .collect()
    }

    pub fn reset_selection(&mut self, library: &Library) {
        self.visible = self.compute_visible(library);
        self.selection = Selection::only(self.visible.first().copied());
        self.leave_preview_if_empty();
    }

    pub fn reconcile(&mut self, library: &Library) {
        self.scope = library.resolve_scope(self.scope);
        self.visible = self.compute_visible(library);
        self.selection.reconcile(&self.visible);
        self.leave_preview_if_empty();
    }

    pub fn prepare_show(&mut self, library: &Library) {
        self.state = TrayState::Browsing;
        self.ctrl_held = false;
        self.kind = None;
        self.query.clear();
        self.scope = library.resolve_scope(self.scope);
        self.reset_selection(library);
    }

    pub fn begin_hide(&mut self, then: Option<Paste>) -> bool {
        match &mut self.state {
            TrayState::Hiding { then: pending } => {
                if then.is_some() {
                    *pending = then;
                }
                false
            }
            _ => {
                self.state = TrayState::Hiding { then };
                true
            }
        }
    }

    pub fn did_hide(&mut self) -> Option<Paste> {
        match std::mem::replace(&mut self.state, TrayState::Hidden) {
            TrayState::Hiding { then } => then,
            _ => None,
        }
    }

    fn leave_preview_if_empty(&mut self) {
        if self.state == TrayState::Previewing && self.selection.primary().is_none() {
            self.state = TrayState::Browsing;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use insert_core::{classify, HistoryLimit, MimeMap, Storage};
    use time::OffsetDateTime;

    fn library_with(texts: &[&str]) -> (tempfile::TempDir, Library) {
        let dir = tempfile::tempdir().unwrap();
        let storage = Storage::new(dir.path().to_path_buf());
        let mut library = Library::open(storage, HistoryLimit::default()).library;
        for text in texts {
            let capture = classify(MimeMap::from_plain_text(text)).unwrap();
            let _ = library.insert(capture, None, OffsetDateTime::now_utc()).unwrap();
        }
        (dir, library)
    }

    #[test]
    fn show_clears_the_filter_and_query_and_selects_the_first_clip() {
        let (_dir, library) = library_with(&["one", "two"]);
        let mut model = TrayModel::new();
        model.kind = Some(ClipKind::Image);
        model.query = "zzz".into();
        model.prepare_show(&library);
        assert_eq!(model.state, TrayState::Browsing);
        assert_eq!(model.kind, None);
        assert_eq!(model.visible.len(), 2);
        assert_eq!(model.selection.primary(), model.visible.first().copied());
    }

    #[test]
    fn preview_ends_when_no_clip_is_left() {
        let (_dir, library) = library_with(&["one"]);
        let mut model = TrayModel::new();
        model.prepare_show(&library);
        model.state = TrayState::Previewing;
        model.query = "nothing matches this".into();
        model.reset_selection(&library);
        assert_eq!(model.state, TrayState::Browsing);
    }

    #[test]
    fn hidden_and_hiding_take_no_keys() {
        let mut model = TrayModel::new();
        assert_eq!(model.key_mode(), None);
        model.state = TrayState::Hiding { then: None };
        assert_eq!(model.key_mode(), None);
        model.state = TrayState::Naming(NameTarget::NewPinboard { pinning: HashSet::new() });
        assert_eq!(model.key_mode(), Some(TrayMode::Naming));
    }

    #[test]
    fn a_hide_finishes_the_activation_that_started_it() {
        let (_dir, library) = library_with(&["one"]);
        let mut model = TrayModel::new();
        model.prepare_show(&library);
        assert!(model.begin_hide(Some(Paste)));
        assert!(!model.begin_hide(None));
        assert_eq!(model.did_hide(), Some(Paste));
        assert_eq!(model.state, TrayState::Hidden);
    }

    #[test]
    fn a_show_during_the_hide_drops_the_pending_paste() {
        let (_dir, library) = library_with(&["one"]);
        let mut model = TrayModel::new();
        model.prepare_show(&library);
        assert!(model.begin_hide(Some(Paste)));
        model.ctrl_held = true;
        model.prepare_show(&library);
        assert!(!model.ctrl_held);
        assert!(model.begin_hide(None));
        assert_eq!(model.did_hide(), None);
    }
}
