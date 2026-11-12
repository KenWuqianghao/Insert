use gtk4 as gtk;
use gtk::{gdk, prelude::*};
use insert_core::{Key, Modifiers};

/// The key-table form of a GDK key press, or `None` for a key the table never names
/// (a modifier key alone, a function key). `first_layout` gives the characters of the key's
/// place on the first keyboard layout, at the levels without and with Shift.
pub fn translate(
    keyval: gdk::Key,
    state: gdk::ModifierType,
    first_layout: impl FnOnce() -> [Option<char>; 2],
) -> Option<(Key, Modifiers)> {
    let modifiers = Modifiers {
        ctrl: state.contains(gdk::ModifierType::CONTROL_MASK),
        shift: state.contains(gdk::ModifierType::SHIFT_MASK),
        alt: state.contains(gdk::ModifierType::ALT_MASK),
        super_key: state.contains(gdk::ModifierType::SUPER_MASK),
    };
    let key = match keyval {
        gdk::Key::Left | gdk::Key::KP_Left => Key::Left,
        gdk::Key::Right | gdk::Key::KP_Right => Key::Right,
        gdk::Key::Up | gdk::Key::KP_Up => Key::Up,
        gdk::Key::Down | gdk::Key::KP_Down => Key::Down,
        gdk::Key::Return => Key::Return,
        gdk::Key::KP_Enter => Key::KeypadEnter,
        gdk::Key::Tab | gdk::Key::ISO_Left_Tab | gdk::Key::KP_Tab => Key::Tab,
        gdk::Key::Escape => Key::Escape,
        gdk::Key::BackSpace => Key::Backspace,
        gdk::Key::Delete | gdk::Key::KP_Delete => Key::Delete,
        gdk::Key::space | gdk::Key::KP_Space => Key::Space,
        other => {
            let typed = other.to_lower().to_unicode().filter(|c| !c.is_control())?;
            match modifiers.ctrl {
                true => Key::Char(shortcut_char(typed, first_layout())),
                false => Key::Char(typed),
            }
        }
    };
    Some((key, modifiers))
}

/// The characters at the levels without and with Shift of `keycode` on the first layout.
pub fn first_layout_chars(display: &gdk::Display, keycode: u32) -> [Option<char>; 2] {
    let keys = display.map_keycode(keycode).unwrap_or_default();
    [0, 1].map(|level| {
        keys.iter()
            .find(|(place, _)| place.group() == 0 && place.level() == level)
            .and_then(|(_, keyval)| keyval.to_unicode())
    })
}

/// The tray shortcuts name Latin letters, digits, and US punctuation. On a Cyrillic layout,
/// Ctrl+C types a Cyrillic letter, and on AZERTY, Ctrl+1 types "&". Then the key's place on the
/// first layout, which is usually Latin, names the shortcut.
fn shortcut_char(typed: char, [plain, shifted]: [Option<char>; 2]) -> char {
    if typed.is_ascii_alphanumeric() {
        return typed;
    }
    if let Some(latin) = plain.filter(char::is_ascii_alphanumeric) {
        return latin.to_ascii_lowercase();
    }
    if let Some(digit) = shifted.filter(char::is_ascii_digit) {
        return digit;
    }
    if typed.is_ascii_graphic() {
        return typed;
    }
    plain.filter(char::is_ascii_graphic).unwrap_or(typed)
}

/// A key for the `test-key` hook, written like "Down", "ctrl+1", "shift+Return", or "space".
pub fn parse_test_key(spec: &str) -> Option<(Key, Modifiers)> {
    let mut modifiers = Modifiers::NONE;
    let mut parts: Vec<&str> = spec.split('+').collect();
    let name = parts.pop()?;
    for part in parts {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" => modifiers.ctrl = true,
            "shift" => modifiers.shift = true,
            "alt" => modifiers.alt = true,
            "super" => modifiers.super_key = true,
            _ => return None,
        }
    }
    let key = match name.to_ascii_lowercase().as_str() {
        "left" => Key::Left,
        "right" => Key::Right,
        "up" => Key::Up,
        "down" => Key::Down,
        "return" | "enter" => Key::Return,
        "tab" => Key::Tab,
        "escape" | "esc" => Key::Escape,
        "backspace" => Key::Backspace,
        "delete" => Key::Delete,
        "space" => Key::Space,
        _ => {
            let mut chars = name.chars();
            let c = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            Key::Char(c)
        }
    };
    Some((key, modifiers))
}

#[cfg(test)]
mod tests {
    use super::*;

    const US_1: [Option<char>; 2] = [Some('1'), Some('!')];

    #[test]
    fn named_keys_and_modifiers_translate() {
        let ctrl = gdk::ModifierType::CONTROL_MASK;
        let shift = gdk::ModifierType::SHIFT_MASK;
        let us = |chars: [char; 2]| move || chars.map(Some);
        assert_eq!(
            translate(gdk::Key::Down, gdk::ModifierType::empty(), || US_1),
            Some((Key::Down, Modifiers::NONE))
        );
        assert_eq!(translate(gdk::Key::_1, ctrl, || US_1), Some((Key::Char('1'), Modifiers::CTRL)));
        assert_eq!(
            translate(gdk::Key::ISO_Left_Tab, shift, || US_1),
            Some((Key::Tab, Modifiers::SHIFT))
        );
        assert_eq!(translate(gdk::Key::A, shift, us(['a', 'A'])), Some((Key::Char('a'), Modifiers::SHIFT)));
        assert_eq!(translate(gdk::Key::comma, ctrl, us([',', '<'])), Some((Key::Char(','), Modifiers::CTRL)));
        assert_eq!(translate(gdk::Key::bracketleft, ctrl, us(['[', '{'])), Some((Key::Char('['), Modifiers::CTRL)));
    }

    #[test]
    fn ctrl_shortcuts_use_the_key_place_on_the_first_layout() {
        let ctrl = gdk::ModifierType::CONTROL_MASK;
        let azerty_1 = || [Some('&'), Some('1')];
        assert_eq!(translate(gdk::Key::ampersand, ctrl, azerty_1), Some((Key::Char('1'), Modifiers::CTRL)));
        let azerty_2 = || [Some('é'), Some('2')];
        assert_eq!(translate(gdk::Key::eacute, ctrl, azerty_2), Some((Key::Char('2'), Modifiers::CTRL)));
        let us_c = || [Some('c'), Some('C')];
        assert_eq!(translate(gdk::Key::Cyrillic_es, ctrl, us_c), Some((Key::Char('c'), Modifiers::CTRL)));
        let us_bracket = || [Some('['), Some('{')];
        assert_eq!(translate(gdk::Key::Cyrillic_ha, ctrl, us_bracket), Some((Key::Char('['), Modifiers::CTRL)));
    }

    #[test]
    fn typing_without_ctrl_keeps_the_layout_character() {
        let none = gdk::ModifierType::empty();
        let us_c = || [Some('c'), Some('C')];
        assert_eq!(translate(gdk::Key::Cyrillic_es, none, us_c), Some((Key::Char('с'), Modifiers::NONE)));
        assert_eq!(translate(gdk::Key::ampersand, none, || [Some('&'), Some('1')]), Some((Key::Char('&'), Modifiers::NONE)));
    }

    #[test]
    fn modifier_keys_alone_are_not_tray_keys() {
        assert_eq!(translate(gdk::Key::Control_L, gdk::ModifierType::empty(), || US_1), None);
        assert_eq!(translate(gdk::Key::F5, gdk::ModifierType::empty(), || US_1), None);
    }
}
