#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Left,
    Right,
    Up,
    Down,
    Return,
    KeypadEnter,
    /// Also `ISO_Left_Tab`, which is what Shift+Tab sends.
    Tab,
    Escape,
    Backspace,
    Delete,
    Space,
    Menu,
    F10,
    /// The key's character without Shift, for example `'q'`, `'1'`, `','`, `'['`.
    Char(char),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub super_key: bool,
}

impl Modifiers {
    pub const NONE: Modifiers = Modifiers {
        ctrl: false,
        shift: false,
        alt: false,
        super_key: false,
    };
    pub const SHIFT: Modifiers = Modifiers {
        shift: true,
        ..Self::NONE
    };
    pub const CTRL: Modifiers = Modifiers {
        ctrl: true,
        ..Self::NONE
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TrayMode {
    Browsing,
    Previewing,
    Naming,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyContext {
    pub mode: TrayMode,
    pub query_is_empty: bool,
    pub composing: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PasteAction {
    Paste,
    PastePlainText,
    Copy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PanelCommand {
    Move(isize),
    Extend(isize),
    SelectAll,
    Activate(PasteAction),
    ActivateIndex(usize),
    DeleteSelection,
    TogglePreview,
    Cancel,
    SwitchScope(isize),
    OpenSettings,
    OpenClipMenu,
    Hide,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mods {
    Exactly(Modifiers),
    Any,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum When {
    Always,
    Naming,
    NotNaming,
    QueryEmpty,
    QueryEmptyOrPreviewing,
}

struct Binding {
    key: Key,
    mods: Mods,
    when: When,
    command: PanelCommand,
}

const fn bind(key: Key, mods: Modifiers, when: When, command: PanelCommand) -> Binding {
    Binding {
        key,
        mods: Mods::Exactly(mods),
        when,
        command,
    }
}

use Key::*;
use PanelCommand::*;
use When::*;
const NONE: Modifiers = Modifiers::NONE;
const SHIFT: Modifiers = Modifiers::SHIFT;
const CTRL: Modifiers = Modifiers::CTRL;

const BINDINGS: &[Binding] = &[
    bind(Char('q'), CTRL, Always, Hide),
    bind(Char('w'), CTRL, Always, Hide),
    Binding {
        key: Escape,
        mods: Mods::Any,
        when: Naming,
        command: Cancel,
    },
    bind(Char('c'), CTRL, NotNaming, Activate(PasteAction::Copy)),
    bind(Char('a'), CTRL, NotNaming, SelectAll),
    bind(Char(','), CTRL, NotNaming, OpenSettings),
    bind(Char('['), CTRL, NotNaming, SwitchScope(-1)),
    bind(Char(']'), CTRL, NotNaming, SwitchScope(1)),
    bind(Backspace, CTRL, NotNaming, DeleteSelection),
    bind(Delete, CTRL, NotNaming, DeleteSelection),
    bind(Char('1'), CTRL, NotNaming, ActivateIndex(0)),
    bind(Char('2'), CTRL, NotNaming, ActivateIndex(1)),
    bind(Char('3'), CTRL, NotNaming, ActivateIndex(2)),
    bind(Char('4'), CTRL, NotNaming, ActivateIndex(3)),
    bind(Char('5'), CTRL, NotNaming, ActivateIndex(4)),
    bind(Char('6'), CTRL, NotNaming, ActivateIndex(5)),
    bind(Char('7'), CTRL, NotNaming, ActivateIndex(6)),
    bind(Char('8'), CTRL, NotNaming, ActivateIndex(7)),
    bind(Char('9'), CTRL, NotNaming, ActivateIndex(8)),
    bind(Left, NONE, NotNaming, Move(-1)),
    bind(Up, NONE, NotNaming, Move(-1)),
    bind(Left, SHIFT, NotNaming, Extend(-1)),
    bind(Up, SHIFT, NotNaming, Extend(-1)),
    bind(Right, NONE, NotNaming, Move(1)),
    bind(Down, NONE, NotNaming, Move(1)),
    bind(Right, SHIFT, NotNaming, Extend(1)),
    bind(Down, SHIFT, NotNaming, Extend(1)),
    bind(Return, NONE, NotNaming, Activate(PasteAction::Paste)),
    bind(KeypadEnter, NONE, NotNaming, Activate(PasteAction::Paste)),
    bind(
        Return,
        SHIFT,
        NotNaming,
        Activate(PasteAction::PastePlainText),
    ),
    bind(
        KeypadEnter,
        SHIFT,
        NotNaming,
        Activate(PasteAction::PastePlainText),
    ),
    bind(Tab, NONE, NotNaming, SwitchScope(1)),
    bind(Tab, SHIFT, NotNaming, SwitchScope(-1)),
    bind(Escape, NONE, NotNaming, Cancel),
    bind(Backspace, NONE, QueryEmpty, DeleteSelection),
    bind(Delete, NONE, QueryEmpty, DeleteSelection),
    bind(Space, NONE, QueryEmptyOrPreviewing, TogglePreview),
    bind(Menu, NONE, NotNaming, OpenClipMenu),
    bind(F10, SHIFT, NotNaming, OpenClipMenu),
];

/// The tray command for a key press, or `None` if the key goes to the focused text field.
pub fn command_for_key(
    key: Key,
    modifiers: Modifiers,
    context: KeyContext,
) -> Option<PanelCommand> {
    if context.composing {
        return None;
    }
    let key = match key {
        Char(c) => Char(c.to_ascii_lowercase()),
        other => other,
    };
    let naming = context.mode == TrayMode::Naming;
    BINDINGS
        .iter()
        .find(|binding| {
            binding.key == key
                && match binding.mods {
                    Mods::Exactly(mods) => mods == modifiers,
                    Mods::Any => true,
                }
                && match binding.when {
                    Always => true,
                    Naming => naming,
                    NotNaming => !naming,
                    QueryEmpty => !naming && context.query_is_empty,
                    QueryEmptyOrPreviewing => {
                        !naming && (context.query_is_empty || context.mode == TrayMode::Previewing)
                    }
                }
        })
        .map(|binding| binding.command)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CancelEffect {
    Browse,
    ClearQuery,
    HideTray,
}

pub fn cancel_effect(mode: TrayMode, query: &str) -> CancelEffect {
    match mode {
        TrayMode::Previewing | TrayMode::Naming => CancelEffect::Browse,
        TrayMode::Browsing if !query.is_empty() => CancelEffect::ClearQuery,
        TrayMode::Browsing => CancelEffect::HideTray,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BROWSING: KeyContext = KeyContext {
        mode: TrayMode::Browsing,
        query_is_empty: true,
        composing: false,
    };
    const SEARCHING: KeyContext = KeyContext {
        query_is_empty: false,
        ..BROWSING
    };
    const PREVIEWING: KeyContext = KeyContext {
        mode: TrayMode::Previewing,
        ..SEARCHING
    };
    const NAMING: KeyContext = KeyContext {
        mode: TrayMode::Naming,
        ..BROWSING
    };

    fn run(key: Key, mods: Modifiers, context: KeyContext) -> Option<PanelCommand> {
        command_for_key(key, mods, context)
    }

    #[test]
    fn ctrl_commands() {
        assert_eq!(
            run(Char('c'), CTRL, BROWSING),
            Some(Activate(PasteAction::Copy))
        );
        assert_eq!(run(Char('a'), CTRL, SEARCHING), Some(SelectAll));
        assert_eq!(run(Char(','), CTRL, BROWSING), Some(OpenSettings));
        assert_eq!(run(Char('['), CTRL, BROWSING), Some(SwitchScope(-1)));
        assert_eq!(run(Char(']'), CTRL, BROWSING), Some(SwitchScope(1)));
        assert_eq!(run(Backspace, CTRL, SEARCHING), Some(DeleteSelection));
        assert_eq!(run(Delete, CTRL, SEARCHING), Some(DeleteSelection));
        assert_eq!(run(Char('1'), CTRL, BROWSING), Some(ActivateIndex(0)));
        assert_eq!(run(Char('9'), CTRL, BROWSING), Some(ActivateIndex(8)));
        assert_eq!(run(Char('0'), CTRL, BROWSING), None);
    }

    #[test]
    fn ctrl_q_and_w_hide_also_while_naming() {
        for context in [BROWSING, NAMING] {
            assert_eq!(run(Char('q'), CTRL, context), Some(Hide));
            assert_eq!(run(Char('W'), CTRL, context), Some(Hide));
        }
    }

    #[test]
    fn edit_keys_and_other_modifiers_go_to_the_field() {
        for c in ['v', 'x', 'z'] {
            assert_eq!(run(Char(c), CTRL, SEARCHING), None);
        }
        let ctrl_shift = Modifiers {
            shift: true,
            ..CTRL
        };
        assert_eq!(run(Char('c'), ctrl_shift, BROWSING), None);
        assert_eq!(run(Left, Modifiers { alt: true, ..NONE }, BROWSING), None);
        assert_eq!(
            run(
                Return,
                Modifiers {
                    super_key: true,
                    ..NONE
                },
                BROWSING
            ),
            None
        );
        assert_eq!(run(Char('a'), NONE, BROWSING), None);
    }

    #[test]
    fn arrows_move_and_shift_arrows_extend() {
        assert_eq!(run(Left, NONE, SEARCHING), Some(Move(-1)));
        assert_eq!(run(Up, NONE, BROWSING), Some(Move(-1)));
        assert_eq!(run(Right, NONE, BROWSING), Some(Move(1)));
        assert_eq!(run(Down, NONE, BROWSING), Some(Move(1)));
        assert_eq!(run(Left, SHIFT, BROWSING), Some(Extend(-1)));
        assert_eq!(run(Down, SHIFT, BROWSING), Some(Extend(1)));
    }

    #[test]
    fn return_pastes_and_shift_return_pastes_plain_text() {
        assert_eq!(
            run(Return, NONE, SEARCHING),
            Some(Activate(PasteAction::Paste))
        );
        assert_eq!(
            run(KeypadEnter, NONE, BROWSING),
            Some(Activate(PasteAction::Paste))
        );
        assert_eq!(
            run(Return, SHIFT, BROWSING),
            Some(Activate(PasteAction::PastePlainText))
        );
        assert_eq!(
            run(KeypadEnter, SHIFT, BROWSING),
            Some(Activate(PasteAction::PastePlainText))
        );
    }

    #[test]
    fn tab_switches_scope_and_esc_cancels() {
        assert_eq!(run(Tab, NONE, BROWSING), Some(SwitchScope(1)));
        assert_eq!(run(Tab, SHIFT, BROWSING), Some(SwitchScope(-1)));
        assert_eq!(run(Escape, NONE, SEARCHING), Some(Cancel));
        assert_eq!(run(Escape, SHIFT, BROWSING), None);
    }

    #[test]
    fn backspace_deletes_only_with_an_empty_query() {
        assert_eq!(run(Backspace, NONE, BROWSING), Some(DeleteSelection));
        assert_eq!(run(Delete, NONE, BROWSING), Some(DeleteSelection));
        assert_eq!(run(Backspace, NONE, SEARCHING), None);
        assert_eq!(run(Delete, NONE, SEARCHING), None);
    }

    #[test]
    fn space_previews_with_an_empty_query_or_while_previewing() {
        assert_eq!(run(Space, NONE, BROWSING), Some(TogglePreview));
        assert_eq!(run(Space, NONE, PREVIEWING), Some(TogglePreview));
        assert_eq!(run(Space, NONE, SEARCHING), None);
        assert_eq!(run(Space, SHIFT, BROWSING), None);
    }

    #[test]
    fn menu_and_shift_f10_open_the_clip_menu() {
        for context in [BROWSING, SEARCHING, PREVIEWING] {
            assert_eq!(run(Menu, NONE, context), Some(OpenClipMenu));
            assert_eq!(run(F10, SHIFT, context), Some(OpenClipMenu));
        }
        assert_eq!(run(F10, NONE, BROWSING), None);
        assert_eq!(run(Menu, NONE, NAMING), None);
        assert_eq!(run(F10, SHIFT, NAMING), None);
    }

    #[test]
    fn naming_passes_keys_to_the_field_except_esc() {
        assert_eq!(run(Escape, NONE, NAMING), Some(Cancel));
        assert_eq!(run(Escape, SHIFT, NAMING), Some(Cancel));
        for key in [Return, Left, Space, Backspace, Tab, Char('c')] {
            assert_eq!(run(key, NONE, NAMING), None, "{key:?}");
        }
        assert_eq!(run(Char('c'), CTRL, NAMING), None);
    }

    #[test]
    fn ime_composition_passes_every_key_to_the_field() {
        let composing = KeyContext {
            composing: true,
            ..BROWSING
        };
        for key in [Return, Space, Left, Escape, Backspace, Char('q')] {
            assert_eq!(run(key, NONE, composing), None, "{key:?}");
        }
        assert_eq!(run(Char('q'), CTRL, composing), None);
    }

    #[test]
    fn esc_order() {
        assert_eq!(
            cancel_effect(TrayMode::Previewing, "abc"),
            CancelEffect::Browse
        );
        assert_eq!(cancel_effect(TrayMode::Naming, ""), CancelEffect::Browse);
        assert_eq!(
            cancel_effect(TrayMode::Browsing, " "),
            CancelEffect::ClearQuery
        );
        assert_eq!(
            cancel_effect(TrayMode::Browsing, ""),
            CancelEffect::HideTray
        );
    }
}
