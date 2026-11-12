use gtk4 as gtk;
use gtk::{gdk, prelude::*};
use insert_core::{Key, Modifiers};

pub fn translate(
    keyval: gdk::Key,
    state: gdk::ModifierType,
    layouts: impl FnOnce() -> Vec<[Option<char>; 2]>,
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
                true => Key::Char(shortcut_char(typed, &layouts())),
                false => Key::Char(typed),
            }
        }
    };
    Some((key, modifiers))
}

/// The key's characters on each layout group, at the levels without and with Shift.
pub fn layout_chars(display: &gdk::Display, keycode: u32) -> Vec<[Option<char>; 2]> {
    let keys = display.map_keycode(keycode).unwrap_or_default();
    let groups = keys.iter().map(|(place, _)| place.group() + 1).max().unwrap_or(0);
    (0..groups)
        .map(|group| {
            [0, 1].map(|level| {
                keys.iter()
                    .find(|(place, _)| place.group() == group && place.level() == level)
                    .and_then(|(_, keyval)| keyval.to_unicode())
            })
        })
        .collect()
}

/// The tray shortcuts name Latin letters, digits, and US punctuation. On AZERTY, Ctrl+1 types
/// "&", so a key whose Shift level is a digit names that digit. On a Cyrillic layout, Ctrl+C types
/// a Cyrillic letter, so the key's place on a Latin layout names the shortcut.
fn shortcut_char(typed: char, layouts: &[[Option<char>; 2]]) -> char {
    if typed.is_ascii_alphanumeric() {
        return typed;
    }
    let shifted_digit = layouts.iter().find_map(|&[plain, shifted]| {
        (plain == Some(typed)).then_some(shifted?).filter(char::is_ascii_digit)
    });
    if let Some(digit) = shifted_digit {
        return digit;
    }
    if typed.is_ascii_graphic() {
        return typed;
    }
    let plain = || layouts.iter().filter_map(|[plain, _]| *plain);
    plain()
        .find(char::is_ascii_alphanumeric)
        .map(|latin| latin.to_ascii_lowercase())
        .or_else(|| plain().find(char::is_ascii_graphic))
        .unwrap_or(typed)
}

#[cfg(feature = "test-hooks")]
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
    use insert_core::{command_for_key, KeyContext, PanelCommand, TrayMode};

    const CTRL: gdk::ModifierType = gdk::ModifierType::CONTROL_MASK;

    fn layouts(groups: &[[char; 2]]) -> impl FnOnce() -> Vec<[Option<char>; 2]> + '_ {
        move || groups.iter().map(|chars| chars.map(Some)).collect()
    }

    fn ctrl_char(keyval: gdk::Key, groups: &[[char; 2]]) -> Option<(Key, Modifiers)> {
        translate(keyval, CTRL, layouts(groups))
    }

    #[test]
    fn named_keys_and_modifiers_translate() {
        let shift = gdk::ModifierType::SHIFT_MASK;
        let us_1 = [['1', '!']];
        assert_eq!(
            translate(gdk::Key::Down, gdk::ModifierType::empty(), layouts(&us_1)),
            Some((Key::Down, Modifiers::NONE))
        );
        assert_eq!(ctrl_char(gdk::Key::_1, &us_1), Some((Key::Char('1'), Modifiers::CTRL)));
        assert_eq!(
            translate(gdk::Key::ISO_Left_Tab, shift, layouts(&us_1)),
            Some((Key::Tab, Modifiers::SHIFT))
        );
        assert_eq!(translate(gdk::Key::A, shift, layouts(&[['a', 'A']])), Some((Key::Char('a'), Modifiers::SHIFT)));
        assert_eq!(ctrl_char(gdk::Key::comma, &[[',', '<']]), Some((Key::Char(','), Modifiers::CTRL)));
        assert_eq!(ctrl_char(gdk::Key::bracketleft, &[['[', '{']]), Some((Key::Char('['), Modifiers::CTRL)));
    }

    #[test]
    fn azerty_digit_keys_name_their_digit() {
        assert_eq!(ctrl_char(gdk::Key::ampersand, &[['&', '1']]), Some((Key::Char('1'), Modifiers::CTRL)));
        assert_eq!(ctrl_char(gdk::Key::eacute, &[['é', '2']]), Some((Key::Char('2'), Modifiers::CTRL)));
    }

    #[test]
    fn altgr_punctuation_keeps_the_typed_character() {
        let switch_back = Some(PanelCommand::SwitchScope(-1));
        let context = KeyContext { mode: TrayMode::Browsing, query_is_empty: true, composing: false };
        let german_8 = ctrl_char(gdk::Key::bracketleft, &[['8', '(']]).unwrap();
        assert_eq!(german_8, (Key::Char('['), Modifiers::CTRL));
        assert_eq!(command_for_key(german_8.0, german_8.1, context), switch_back);
        let french_5 = ctrl_char(gdk::Key::bracketleft, &[['(', '5']]).unwrap();
        assert_eq!(french_5, (Key::Char('['), Modifiers::CTRL));
    }

    #[test]
    fn non_latin_letters_use_the_key_place_on_a_latin_layout() {
        let es = (Key::Char('c'), Modifiers::CTRL);
        assert_eq!(ctrl_char(gdk::Key::Cyrillic_es, &[['c', 'C'], ['с', 'С']]), Some(es));
        assert_eq!(ctrl_char(gdk::Key::Cyrillic_es, &[['с', 'С'], ['c', 'C']]), Some(es));
        let ha = ctrl_char(gdk::Key::Cyrillic_ha, &[['х', 'Х'], ['[', '{']]);
        assert_eq!(ha, Some((Key::Char('['), Modifiers::CTRL)));
    }

    #[test]
    fn typing_without_ctrl_keeps_the_layout_character() {
        let none = gdk::ModifierType::empty();
        let cyrillic = translate(gdk::Key::Cyrillic_es, none, layouts(&[['c', 'C'], ['с', 'С']]));
        assert_eq!(cyrillic, Some((Key::Char('с'), Modifiers::NONE)));
        let ampersand = translate(gdk::Key::ampersand, none, layouts(&[['&', '1']]));
        assert_eq!(ampersand, Some((Key::Char('&'), Modifiers::NONE)));
    }

    #[test]
    fn modifier_keys_alone_are_not_tray_keys() {
        let none = gdk::ModifierType::empty();
        assert_eq!(translate(gdk::Key::Control_L, none, layouts(&[])), None);
        assert_eq!(translate(gdk::Key::F5, none, layouts(&[])), None);
    }
}
