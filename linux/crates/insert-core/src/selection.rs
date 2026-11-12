use std::collections::HashSet;

use crate::model::ClipId;

/// The selected cards. Every method takes the visible clip ids in display order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    ids: HashSet<ClipId>,
    primary: Option<ClipId>,
    anchor: Option<ClipId>,
}

impl Selection {
    pub fn only(id: Option<ClipId>) -> Self {
        Self {
            ids: id.into_iter().collect(),
            primary: id,
            anchor: id,
        }
    }

    pub fn ids(&self) -> &HashSet<ClipId> {
        &self.ids
    }

    pub fn primary(&self) -> Option<ClipId> {
        self.primary
    }

    pub fn anchor(&self) -> Option<ClipId> {
        self.anchor
    }

    pub fn contains(&self, id: ClipId) -> bool {
        self.ids.contains(&id)
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    pub fn move_by(&mut self, offset: isize, visible: &[ClipId]) {
        *self = Self::only(self.neighbor(offset, visible));
    }

    pub fn extend_by(&mut self, offset: isize, visible: &[ClipId]) {
        match self.neighbor(offset, visible) {
            Some(target) => self.select_range(target, visible),
            None => *self = Self::default(),
        }
    }

    pub fn toggle(&mut self, id: ClipId, visible: &[ClipId]) {
        if self.ids.remove(&id) {
            if self.primary == Some(id) {
                self.primary = visible.iter().copied().find(|id| self.ids.contains(id));
            }
            self.anchor = self.primary;
        } else {
            self.ids.insert(id);
            self.primary = Some(id);
            self.anchor = self.anchor.or(Some(id));
        }
    }

    pub fn select_range(&mut self, id: ClipId, visible: &[ClipId]) {
        let position = |id: ClipId| visible.iter().position(|visible| *visible == id);
        let anchor = self.anchor.or(self.primary);
        let (Some(target), Some(anchor), Some(start)) =
            (position(id), anchor, anchor.and_then(position))
        else {
            *self = Self::only(Some(id));
            return;
        };
        self.ids = visible[start.min(target)..=start.max(target)]
            .iter()
            .copied()
            .collect();
        self.primary = Some(id);
        self.anchor = Some(anchor);
    }

    pub fn select_all(&mut self, visible: &[ClipId]) {
        self.ids = visible.iter().copied().collect();
        self.primary = self
            .primary
            .filter(|id| self.ids.contains(id))
            .or(visible.first().copied());
        self.anchor = visible.first().copied();
    }

    pub fn reconcile(&mut self, visible: &[ClipId]) {
        let survivors: Vec<ClipId> = visible
            .iter()
            .copied()
            .filter(|id| self.ids.contains(id))
            .collect();
        let Some(first) = survivors.first().copied() else {
            *self = Self::only(visible.first().copied());
            return;
        };
        self.ids = survivors.into_iter().collect();
        if !self.primary.is_some_and(|id| self.ids.contains(&id)) {
            self.primary = Some(first);
        }
        if !self.anchor.is_some_and(|id| self.ids.contains(&id)) {
            self.anchor = self.primary;
        }
    }

    pub fn after_deleting(&self, removed: &HashSet<ClipId>, visible: &[ClipId]) -> Self {
        if !self.primary.is_some_and(|id| removed.contains(&id)) {
            let mut kept = self.clone();
            kept.ids.retain(|id| !removed.contains(id));
            if kept.anchor.is_some_and(|id| removed.contains(&id)) {
                kept.anchor = kept.primary;
            }
            return kept;
        }
        let remaining: Vec<ClipId> = visible
            .iter()
            .copied()
            .filter(|id| !removed.contains(id))
            .collect();
        if remaining.is_empty() {
            return Self::default();
        }
        let first_removed = visible
            .iter()
            .position(|id| removed.contains(id))
            .unwrap_or(0);
        Self::only(Some(remaining[first_removed.min(remaining.len() - 1)]))
    }

    fn neighbor(&self, offset: isize, visible: &[ClipId]) -> Option<ClipId> {
        if visible.is_empty() {
            return None;
        }
        let current = self
            .primary
            .and_then(|id| visible.iter().position(|v| *v == id))
            .unwrap_or(0) as isize;
        let index = (current + offset).clamp(0, visible.len() as isize - 1) as usize;
        Some(visible[index])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(count: usize) -> Vec<ClipId> {
        (0..count).map(|_| ClipId::new()).collect()
    }

    fn set(ids: &[ClipId]) -> HashSet<ClipId> {
        ids.iter().copied().collect()
    }

    #[test]
    fn move_clamps_and_starts_at_index_zero() {
        let v = ids(3);
        let mut selection = Selection::default();
        selection.move_by(1, &v);
        assert_eq!(selection, Selection::only(Some(v[1])));
        selection.move_by(5, &v);
        assert_eq!(selection.primary(), Some(v[2]));
        selection.move_by(-9, &v);
        assert_eq!(selection.primary(), Some(v[0]));
        selection.move_by(1, &[]);
        assert!(selection.is_empty());
    }

    #[test]
    fn extend_selects_from_anchor_to_target() {
        let v = ids(4);
        let mut selection = Selection::only(Some(v[1]));
        selection.extend_by(1, &v);
        selection.extend_by(1, &v);
        assert_eq!(selection.ids(), &set(&v[1..4]));
        assert_eq!(
            (selection.primary(), selection.anchor()),
            (Some(v[3]), Some(v[1]))
        );
        selection.extend_by(-3, &v);
        assert_eq!(selection.ids(), &set(&v[0..2]));
    }

    #[test]
    fn select_range_without_anchor_selects_only_the_target() {
        let v = ids(3);
        let mut selection = Selection::default();
        selection.select_range(v[2], &v);
        assert_eq!(selection, Selection::only(Some(v[2])));
        let mut hidden_anchor = Selection::only(Some(ClipId::new()));
        hidden_anchor.select_range(v[1], &v);
        assert_eq!(hidden_anchor, Selection::only(Some(v[1])));
    }

    #[test]
    fn toggle_adds_and_removes_and_moves_the_primary() {
        let v = ids(3);
        let mut selection = Selection::only(Some(v[0]));
        selection.toggle(v[2], &v);
        assert_eq!(selection.ids(), &set(&[v[0], v[2]]));
        assert_eq!(
            (selection.primary(), selection.anchor()),
            (Some(v[2]), Some(v[0]))
        );
        selection.toggle(v[2], &v);
        assert_eq!(
            (selection.primary(), selection.anchor()),
            (Some(v[0]), Some(v[0]))
        );
        selection.toggle(v[0], &v);
        assert!(selection.is_empty());
        assert_eq!(selection.primary(), None);
    }

    #[test]
    fn select_all_keeps_a_visible_primary() {
        let v = ids(3);
        let mut selection = Selection::only(Some(v[1]));
        selection.select_all(&v);
        assert_eq!(selection.ids(), &set(&v));
        assert_eq!(
            (selection.primary(), selection.anchor()),
            (Some(v[1]), Some(v[0]))
        );
    }

    #[test]
    fn reconcile_keeps_survivors_or_selects_the_first() {
        let v = ids(4);
        let mut selection = Selection::default();
        selection.select_range(v[1], &v);
        selection.select_range(v[3], &v);
        selection.reconcile(&[v[0], v[1], v[2]]);
        assert_eq!(selection.ids(), &set(&v[1..3]));
        assert_eq!(
            (selection.primary(), selection.anchor()),
            (Some(v[1]), Some(v[1]))
        );
        selection.reconcile(&[v[0], v[3]]);
        assert_eq!(selection, Selection::only(Some(v[0])));
    }

    #[test]
    fn after_deleting_the_primary_selects_the_next_card() {
        let v = ids(4);
        let mut selection = Selection::only(Some(v[1]));
        selection.extend_by(1, &v);
        let next = selection.after_deleting(&set(&v[1..3]), &v);
        assert_eq!(next, Selection::only(Some(v[3])));
        let last = Selection::only(Some(v[3])).after_deleting(&set(&[v[3]]), &v);
        assert_eq!(last, Selection::only(Some(v[2])));
        assert_eq!(
            Selection::only(Some(v[0])).after_deleting(&set(&v), &v),
            Selection::default()
        );
    }

    #[test]
    fn after_deleting_others_keeps_the_primary() {
        let v = ids(3);
        let mut selection = Selection::only(Some(v[0]));
        selection.select_range(v[2], &v);
        let next = selection.after_deleting(&set(&[v[0]]), &v);
        assert_eq!(next.ids(), &set(&v[1..3]));
        assert_eq!((next.primary(), next.anchor()), (Some(v[2]), Some(v[2])));
    }
}
