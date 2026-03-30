use crossterm::event::{KeyCode, KeyModifiers};

use super::TerminalKey;

#[allow(dead_code)] // Next step: raw stdin parser will feed TerminalKey directly through this path.
pub fn parse_terminal_key_sequence(data: &str) -> Option<TerminalKey> {
    parse_kitty_key_sequence(data)
        .or_else(|| parse_modify_other_keys_sequence(data))
        .or_else(|| parse_legacy_key_sequence(data))
}

#[allow(dead_code)] // Reserved for the upcoming raw stdin parser.
fn parse_kitty_key_sequence(data: &str) -> Option<TerminalKey> {
    let body = data.strip_prefix("\x1b[")?.strip_suffix('u')?;

    let (main, event_type) = match body.rsplit_once(':') {
        Some((head, tail)) if tail.chars().all(|ch| ch.is_ascii_digit()) && head.contains(';') => {
            (head, Some(tail))
        }
        _ => (body, None),
    };

    let (key_part, modifier_part) = main.rsplit_once(';').unwrap_or((main, "1"));
    let modifier = modifier_part.parse::<u8>().ok()?.checked_sub(1)?;

    let mut key_fields = key_part.split(':');
    let codepoint = key_fields.next()?.parse::<u32>().ok()?;
    let shifted_codepoint = key_fields
        .next()
        .filter(|field| !field.is_empty())
        .and_then(|field| field.parse::<u32>().ok());

    let code = kitty_codepoint_to_keycode(codepoint)?;
    let kind = parse_kitty_event_type(event_type)?;

    Some(TerminalKey {
        code,
        modifiers: key_modifiers_from_u8(modifier),
        kind,
        shifted_codepoint,
    })
}

#[allow(dead_code)] // Reserved for the upcoming raw stdin parser.
fn parse_modify_other_keys_sequence(data: &str) -> Option<TerminalKey> {
    let body = data.strip_prefix("\x1b[27;")?.strip_suffix('~')?;
    let (modifier_part, codepoint_part) = body.split_once(';')?;
    let modifier = modifier_part.parse::<u8>().ok()?.checked_sub(1)?;
    let codepoint = codepoint_part.parse::<u32>().ok()?;

    Some(TerminalKey::new(
        kitty_codepoint_to_keycode(codepoint)?,
        key_modifiers_from_u8(modifier),
    ))
}

#[allow(dead_code)] // Reserved for the upcoming raw stdin parser.
fn parse_legacy_key_sequence(data: &str) -> Option<TerminalKey> {
    if let Some(key) = parse_legacy_special_sequence(data) {
        return Some(key);
    }

    match data {
        "\r" | "\n" => Some(TerminalKey::new(KeyCode::Enter, KeyModifiers::empty())),
        "\t" => Some(TerminalKey::new(KeyCode::Tab, KeyModifiers::empty())),
        "\x1b" => Some(TerminalKey::new(KeyCode::Esc, KeyModifiers::empty())),
        "\x7f" => Some(TerminalKey::new(KeyCode::Backspace, KeyModifiers::empty())),
        _ if data.starts_with('\x1b') => {
            let rest = data.strip_prefix('\x1b')?;
            if rest.chars().count() == 1 {
                let ch = rest.chars().next()?;
                Some(TerminalKey::new(KeyCode::Char(ch), KeyModifiers::ALT))
            } else {
                None
            }
        }
        _ if data.chars().count() == 1 => {
            let ch = data.chars().next()?;

            if let Some(ctrl_key) = parse_legacy_ctrl_char(ch) {
                return Some(ctrl_key);
            }

            let mut modifiers = KeyModifiers::empty();
            let code = if ch.is_ascii_uppercase() {
                modifiers |= KeyModifiers::SHIFT;
                KeyCode::Char(ch)
            } else {
                KeyCode::Char(ch)
            };
            Some(TerminalKey::new(code, modifiers))
        }
        _ => None,
    }
}

fn parse_legacy_ctrl_char(ch: char) -> Option<TerminalKey> {
    match ch as u32 {
        0 => Some(TerminalKey::new(KeyCode::Char(' '), KeyModifiers::CONTROL)),
        1..=26 => Some(TerminalKey::new(
            KeyCode::Char(char::from_u32((ch as u32) + 96)?),
            KeyModifiers::CONTROL,
        )),
        27 => Some(TerminalKey::new(KeyCode::Char('['), KeyModifiers::CONTROL)),
        28 => Some(TerminalKey::new(KeyCode::Char('\\'), KeyModifiers::CONTROL)),
        29 => Some(TerminalKey::new(KeyCode::Char(']'), KeyModifiers::CONTROL)),
        30 => Some(TerminalKey::new(KeyCode::Char('^'), KeyModifiers::CONTROL)),
        31 => Some(TerminalKey::new(KeyCode::Char('-'), KeyModifiers::CONTROL)),
        _ => None,
    }
}

fn parse_legacy_special_sequence(data: &str) -> Option<TerminalKey> {
    match data {
        "\x1b\x1b[A" => Some(TerminalKey::new(KeyCode::Up, KeyModifiers::ALT)),
        "\x1b\x1b[B" => Some(TerminalKey::new(KeyCode::Down, KeyModifiers::ALT)),
        "\x1b\x1b[C" => Some(TerminalKey::new(KeyCode::Right, KeyModifiers::ALT)),
        "\x1b\x1b[D" => Some(TerminalKey::new(KeyCode::Left, KeyModifiers::ALT)),
        "\x1b[A" => Some(TerminalKey::new(KeyCode::Up, KeyModifiers::empty())),
        "\x1b[B" => Some(TerminalKey::new(KeyCode::Down, KeyModifiers::empty())),
        "\x1b[C" => Some(TerminalKey::new(KeyCode::Right, KeyModifiers::empty())),
        "\x1b[D" => Some(TerminalKey::new(KeyCode::Left, KeyModifiers::empty())),
        "\x1b[H" | "\x1bOH" | "\x1b[1~" | "\x1b[7~" => {
            Some(TerminalKey::new(KeyCode::Home, KeyModifiers::empty()))
        }
        "\x1b[F" | "\x1bOF" | "\x1b[4~" | "\x1b[8~" => {
            Some(TerminalKey::new(KeyCode::End, KeyModifiers::empty()))
        }
        "\x1b[5~" => Some(TerminalKey::new(KeyCode::PageUp, KeyModifiers::empty())),
        "\x1b[6~" => Some(TerminalKey::new(KeyCode::PageDown, KeyModifiers::empty())),
        "\x1b[2~" => Some(TerminalKey::new(KeyCode::Insert, KeyModifiers::empty())),
        "\x1b[3~" => Some(TerminalKey::new(KeyCode::Delete, KeyModifiers::empty())),
        "\x1b[Z" => Some(TerminalKey::new(KeyCode::BackTab, KeyModifiers::SHIFT)),
        _ => parse_xterm_modified_special_sequence(data),
    }
}

fn parse_xterm_modified_special_sequence(data: &str) -> Option<TerminalKey> {
    let body = data.strip_prefix("\x1b[")?;

    if let Some(body) = body.strip_prefix("1;") {
        let suffix_char = body.chars().last()?;
        if suffix_char.is_ascii_alphabetic() {
            let modifier = body.strip_suffix(suffix_char)?;
            let mod_value = modifier.parse::<u8>().ok()?.checked_sub(1)?;
            let code = match suffix_char {
                'A' => KeyCode::Up,
                'B' => KeyCode::Down,
                'C' => KeyCode::Right,
                'D' => KeyCode::Left,
                'H' => KeyCode::Home,
                'F' => KeyCode::End,
                _ => return None,
            };
            return Some(TerminalKey::new(code, key_modifiers_from_u8(mod_value)));
        }
    }

    let tilde_body = body.strip_suffix('~')?;
    let (code_part, modifier_part) = tilde_body.split_once(';')?;
    let mod_value = modifier_part.parse::<u8>().ok()?.checked_sub(1)?;
    let code = match code_part {
        "2" => KeyCode::Insert,
        "3" => KeyCode::Delete,
        "5" => KeyCode::PageUp,
        "6" => KeyCode::PageDown,
        _ => return None,
    };
    Some(TerminalKey::new(code, key_modifiers_from_u8(mod_value)))
}

#[allow(dead_code)] // Reserved for the upcoming raw stdin parser.
fn parse_kitty_event_type(value: Option<&str>) -> Option<crossterm::event::KeyEventKind> {
    match value.unwrap_or("1") {
        "1" => Some(crossterm::event::KeyEventKind::Press),
        "2" => Some(crossterm::event::KeyEventKind::Repeat),
        "3" => Some(crossterm::event::KeyEventKind::Release),
        _ => None,
    }
}

#[allow(dead_code)] // Reserved for the upcoming raw stdin parser.
fn kitty_codepoint_to_keycode(codepoint: u32) -> Option<KeyCode> {
    match codepoint {
        8 | 127 => Some(KeyCode::Backspace),
        9 => Some(KeyCode::Tab),
        13 | 57414 => Some(KeyCode::Enter),
        27 => Some(KeyCode::Esc),
        57417 => Some(KeyCode::Left),
        57418 => Some(KeyCode::Right),
        57419 => Some(KeyCode::Up),
        57420 => Some(KeyCode::Down),
        57421 => Some(KeyCode::PageUp),
        57422 => Some(KeyCode::PageDown),
        57423 => Some(KeyCode::Home),
        57424 => Some(KeyCode::End),
        57425 => Some(KeyCode::Insert),
        57426 => Some(KeyCode::Delete),
        value => char::from_u32(value).map(KeyCode::Char),
    }
}

#[allow(dead_code)] // Reserved for the upcoming raw stdin parser.
fn key_modifiers_from_u8(modifier: u8) -> KeyModifiers {
    let mut mods = KeyModifiers::empty();
    if modifier & 0b0000_0001 != 0 {
        mods |= KeyModifiers::SHIFT;
    }
    if modifier & 0b0000_0010 != 0 {
        mods |= KeyModifiers::ALT;
    }
    if modifier & 0b0000_0100 != 0 {
        mods |= KeyModifiers::CONTROL;
    }
    if modifier & 0b0000_1000 != 0 {
        mods |= KeyModifiers::SUPER;
    }
    if modifier & 0b0001_0000 != 0 {
        mods |= KeyModifiers::HYPER;
    }
    if modifier & 0b0010_0000 != 0 {
        mods |= KeyModifiers::META;
    }
    mods
}
