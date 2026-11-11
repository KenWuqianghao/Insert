use gtk4 as gtk;
use gtk::gdk;
use insert_core::{Key, Modifiers};

/// The key-table form of a GDK key press, or `None` for a key the table never names
/// (a modifier key alone, a function key).
pub fn translate(keyval: gdk::Key, state: gdk::ModifierType) -> Option<(Key, Modifiers)> {
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
        other => Key::Char(other.to_lower().to_unicode().filter(|c| !c.is_control())?),
    };
    let modifiers = Modifiers {
        ctrl: state.contains(gdk::ModifierType::CONTROL_MASK),
        shift: state.contains(gdk::ModifierType::SHIFT_MASK),
        alt: state.contains(gdk::ModifierType::ALT_MASK),
        super_key: state.contains(gdk::ModifierType::SUPER_MASK),
    };
    Some((key, modifiers))
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

    #[test]
    fn named_keys_and_modifiers_translate() {
        let ctrl = gdk::ModifierType::CONTROL_MASK;
        let shift = gdk::ModifierType::SHIFT_MASK;
        assert_eq!(
            translate(gdk::Key::Down, gdk::ModifierType::empty()),
            Some((Key::Down, Modifiers::NONE))
        );
        assert_eq!(translate(gdk::Key::_1, ctrl), Some((Key::Char('1'), Modifiers::CTRL)));
        assert_eq!(
            translate(gdk::Key::ISO_Left_Tab, shift),
            Some((Key::Tab, Modifiers::SHIFT))
        );
        assert_eq!(translate(gdk::Key::A, shift), Some((Key::Char('a'), Modifiers::SHIFT)));
        assert_eq!(translate(gdk::Key::comma, ctrl), Some((Key::Char(','), Modifiers::CTRL)));
    }

    #[test]
    fn modifier_keys_alone_are_not_tray_keys() {
        assert_eq!(translate(gdk::Key::Control_L, gdk::ModifierType::empty()), None);
        assert_eq!(translate(gdk::Key::F5, gdk::ModifierType::empty()), None);
    }
}
