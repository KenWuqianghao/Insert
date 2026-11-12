use std::collections::HashSet;

use insert_core::{ClipId, ClipKind, Library, NameTarget, PasteAction, Scope, Selection, TrayMode};

/// The tray's life cycle. Hidden and Hiding take no keys.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrayState {
    Hidden,
    Browsing,
    Previewing,
    Naming(NameTarget),
    /// `then` runs when the hide ends. A show during the hide drops it.
    Hiding { then: Option<Activation> },
}

/// A clip that the user chose. When the tray is gone, the clip moves to the front, and the paste
/// keys go out if the clipboard has the clip (`copied`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Activation {
    pub id: ClipId,
    pub action: PasteAction,
    pub copied: bool,
}

#[derive(Debug)]
pub struct TrayModel {
    pub state: TrayState,
    /// Kept across opens, not across restarts.
    pub scope: Scope,
    pub kind: Option<ClipKind>,
    pub query: String,
    pub selection: Selection,
    pub ctrl_held: bool,
    /// Clips that play the delete animation now. A second delete waits for it.
    pub deleting: HashSet<ClipId>,
    /// The clips that the open card menu acts on.
    pub menu_targets: HashSet<ClipId>,
    /// The visible ids that the strip shows now.
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

    /// The mode for the key table, or `None` when the tray takes no keys.
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

    /// After a scope, query, or filter change: select the first visible clip.
    pub fn reset_selection(&mut self, library: &Library) {
        self.visible = self.compute_visible(library);
        self.selection = Selection::only(self.visible.first().copied());
        self.leave_preview_if_empty();
    }

    /// After a library change: keep what survives.
    pub fn reconcile(&mut self, library: &Library) {
        self.scope = library.resolve_scope(self.scope);
        self.visible = self.compute_visible(library);
        self.selection.reconcile(&self.visible);
        self.leave_preview_if_empty();
    }

    pub fn prepare_show(&mut self, library: &Library) {
        self.state = TrayState::Browsing;
        self.kind = None;
        self.query.clear();
        self.scope = library.resolve_scope(self.scope);
        self.reset_selection(library);
    }

    /// Starts a hide of a shown tray. Returns false when a hide runs already; that hide takes
    /// `then` if it is not `None`. A hidden tray must not call this.
    pub fn begin_hide(&mut self, then: Option<Activation>) -> bool {
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

    /// Ends a running hide. Returns the activation to finish, if any.
    pub fn did_hide(&mut self) -> Option<Activation> {
        self.ctrl_held = false;
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

    fn activation(library: &Library) -> Activation {
        Activation {
            id: library.visible(Scope::History, None, "")[0].id,
            action: PasteAction::Paste,
            copied: true,
        }
    }

    #[test]
    fn a_hide_finishes_the_activation_that_started_it() {
        let (_dir, library) = library_with(&["one"]);
        let mut model = TrayModel::new();
        model.prepare_show(&library);
        let paste = activation(&library);
        assert!(model.begin_hide(Some(paste)));
        assert!(!model.begin_hide(None));
        assert_eq!(model.did_hide(), Some(paste));
        assert_eq!(model.state, TrayState::Hidden);
    }

    #[test]
    fn a_show_during_the_hide_drops_the_pending_paste() {
        let (_dir, library) = library_with(&["one"]);
        let mut model = TrayModel::new();
        model.prepare_show(&library);
        assert!(model.begin_hide(Some(activation(&library))));
        model.prepare_show(&library);
        assert!(model.begin_hide(None));
        assert_eq!(model.did_hide(), None);
    }
}
