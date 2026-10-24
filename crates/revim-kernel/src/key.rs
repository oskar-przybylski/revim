//! Keys: a terminal-independent key type and Vim-style key notation.

use std::fmt;

/// A key press, independent of the terminal library.
///
/// Written and displayed in Vim notation: `j`, `<C-q>`, `<Up>`, `<CR>` (see [`parse_key`]).
/// Shift is part of the character (`J`), not a separate modifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Key {
    /// A printable character, including shifted ones (`J`, `:`).
    Char(char),
    /// A character pressed with Ctrl (`<C-q>`); letters are lowercase.
    Ctrl(char),
    Up,
    Down,
    Left,
    Right,
    Esc,
    Enter,
    Backspace,
    Tab,
}

/// Why a key notation could not be parsed by [`parse_key`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyParseError {
    /// The notation is an empty string.
    Empty,
    /// A `<...>` name that is not a known key; holds the whole notation.
    UnknownKey(String),
    /// The notation describes more than one key (`gg`); holds the whole notation.
    SequenceNotSupported(String),
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Key::Char(' ') => f.write_str("<Space>"),
            Key::Char('<') => f.write_str("<lt>"),
            Key::Char(c) => write!(f, "{c}"),
            Key::Ctrl(c) => write!(f, "<C-{c}>"),
            Key::Up => f.write_str("<Up>"),
            Key::Down => f.write_str("<Down>"),
            Key::Left => f.write_str("<Left>"),
            Key::Right => f.write_str("<Right>"),
            Key::Esc => f.write_str("<Esc>"),
            Key::Enter => f.write_str("<CR>"),
            Key::Backspace => f.write_str("<BS>"),
            Key::Tab => f.write_str("<Tab>"),
        }
    }
}

impl fmt::Display for KeyParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            KeyParseError::Empty => "empty key",
            KeyParseError::UnknownKey(_) => "unknown key name",
            KeyParseError::SequenceNotSupported(_) => "key sequences are not supported yet",
        })
    }
}

/// Parses a single key written in Vim-style notation.
///
/// Accepted forms:
/// - a single character: `a`, `G`, `<`, `>`, `:` → [`Key::Char`]
/// - `<C-x>` with exactly one character after `C-` → [`Key::Ctrl`]; ASCII
///   letters are normalized to lowercase, so `<C-A>` equals `<C-a>`
/// - named keys: `<Up>`, `<Down>`, `<Left>`, `<Right>`, `<Esc>`,
///   `<CR>` / `<Enter>` / `<Return>`, `<BS>` / `<Backspace>`, `<Tab>`
/// - `<Space>` → `Key::Char(' ')`, `<lt>` → `Key::Char('<')`
///
/// Names and the `C-` prefix are case-insensitive.
///
/// # Errors
///
/// - [`KeyParseError::Empty`] for an empty string
/// - [`KeyParseError::UnknownKey`] for an unrecognized `<...>` name,
///   including `<>` and `<C->`
/// - [`KeyParseError::SequenceNotSupported`] for anything describing more
///   than one key, e.g. `gg` or `<C-a>b`
///
/// ```
/// use revim_kernel::key::{Key, parse_key};
///
/// assert_eq!(parse_key("j"), Ok(Key::Char('j')));
/// assert_eq!(parse_key("<C-q>"), Ok(Key::Ctrl('q')));
/// assert_eq!(parse_key("<Up>"), Ok(Key::Up));
/// assert!(parse_key("gg").is_err());
/// ```
pub fn parse_key(s: &str) -> Result<Key, KeyParseError> {
    let mut chars = s.chars();
    let first = chars.next().ok_or(KeyParseError::Empty)?;
    if chars.next().is_none() {
        return Ok(Key::Char(first));
    }

    let inner = s
        .strip_prefix('<')
        .and_then(|rest| rest.strip_suffix('>'))
        .filter(|inner| !inner.contains(['<', '>']))
        .ok_or_else(|| KeyParseError::SequenceNotSupported(s.to_string()))?;

    parse_bracketed(inner).ok_or_else(|| KeyParseError::UnknownKey(s.to_string()))
}

/// Parses the content of a `<...>` key, without the brackets.
fn parse_bracketed(inner: &str) -> Option<Key> {
    if let Some(rest) = strip_prefix_ignore_case(inner, "C-") {
        return single_char(rest).map(|c| Key::Ctrl(c.to_ascii_lowercase()));
    }

    const NAMED: &[(&str, Key)] = &[
        ("up", Key::Up),
        ("down", Key::Down),
        ("left", Key::Left),
        ("right", Key::Right),
        ("esc", Key::Esc),
        ("cr", Key::Enter),
        ("enter", Key::Enter),
        ("return", Key::Enter),
        ("bs", Key::Backspace),
        ("backspace", Key::Backspace),
        ("tab", Key::Tab),
        ("space", Key::Char(' ')),
        ("lt", Key::Char('<')),
    ];

    NAMED
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(inner))
        .map(|&(_, key)| key)
}

fn strip_prefix_ignore_case<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let head = s.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &s[prefix.len()..])
}

fn single_char(s: &str) -> Option<char> {
    let mut chars = s.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => Some(c),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_chars() {
        assert_eq!(parse_key("a"), Ok(Key::Char('a')));
        assert_eq!(parse_key("G"), Ok(Key::Char('G')));
        assert_eq!(parse_key("<"), Ok(Key::Char('<')));
        assert_eq!(parse_key(">"), Ok(Key::Char('>')));
        assert_eq!(parse_key("ż"), Ok(Key::Char('ż')));
    }

    #[test]
    fn ctrl_is_case_insensitive() {
        assert_eq!(parse_key("<C-a>"), Ok(Key::Ctrl('a')));
        assert_eq!(parse_key("<c-A>"), Ok(Key::Ctrl('a')));
        assert_eq!(parse_key("<C-[>"), Ok(Key::Ctrl('[')));
    }

    #[test]
    fn named_keys() {
        assert_eq!(parse_key("<Esc>"), Ok(Key::Esc));
        assert_eq!(parse_key("<ESC>"), Ok(Key::Esc));
        assert_eq!(parse_key("<CR>"), Ok(Key::Enter));
        assert_eq!(parse_key("<Enter>"), Ok(Key::Enter));
        assert_eq!(parse_key("<bs>"), Ok(Key::Backspace));
        assert_eq!(parse_key("<Tab>"), Ok(Key::Tab));
        assert_eq!(parse_key("<Up>"), Ok(Key::Up));
        assert_eq!(parse_key("<Space>"), Ok(Key::Char(' ')));
        assert_eq!(parse_key("<lt>"), Ok(Key::Char('<')));
    }

    #[test]
    fn empty() {
        assert_eq!(parse_key(""), Err(KeyParseError::Empty));
    }

    #[test]
    fn unknown_keys() {
        for s in ["<Foo>", "<>", "<C->", "<C-ab>", "<żółw>"] {
            assert_eq!(
                parse_key(s),
                Err(KeyParseError::UnknownKey(s.into())),
                "{s}"
            );
        }
    }

    #[test]
    fn display_uses_vim_notation() {
        assert_eq!(Key::Char('j').to_string(), "j");
        assert_eq!(Key::Ctrl('q').to_string(), "<C-q>");
        assert_eq!(Key::Enter.to_string(), "<CR>");
        assert_eq!(Key::Char(' ').to_string(), "<Space>");
        assert_eq!(Key::Char('<').to_string(), "<lt>");
    }

    #[test]
    fn display_roundtrips_through_parse() {
        let keys = [
            Key::Char('a'),
            Key::Char('G'),
            Key::Char(' '),
            Key::Char('<'),
            Key::Char('>'),
            Key::Char(':'),
            Key::Ctrl('q'),
            Key::Up,
            Key::Down,
            Key::Left,
            Key::Right,
            Key::Esc,
            Key::Enter,
            Key::Backspace,
            Key::Tab,
        ];
        for key in keys {
            assert_eq!(parse_key(&key.to_string()), Ok(key), "{key}");
        }
    }

    #[test]
    fn parse_error_display() {
        assert_eq!(KeyParseError::Empty.to_string(), "empty key");
        assert_eq!(
            KeyParseError::UnknownKey("<Foo>".into()).to_string(),
            "unknown key name"
        );
    }

    #[test]
    fn sequences() {
        for s in ["gg", "<Esc>a", "a<Esc>", "<C-a><C-b>", "<abc", "abc>"] {
            assert_eq!(
                parse_key(s),
                Err(KeyParseError::SequenceNotSupported(s.into())),
                "{s}"
            );
        }
    }
}
