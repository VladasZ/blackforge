//! The one rule for a username, used by the window before it asks and by the
//! server before it stores.

use thiserror::Error;

pub const MIN_LENGTH: usize = 3;
pub const MAX_LENGTH: usize = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum UsernameError {
    #[error("a username has at least {MIN_LENGTH} characters")]
    TooShort,
    #[error("a username has at most {MAX_LENGTH} characters")]
    TooLong,
    #[error("a username has only Latin letters, digits and the underscore")]
    BadCharacter,
}

/// The name as it is stored and compared, in lower case, so `Vladas` and
/// `vladas` are one name. Other scripts are refused, a lookalike letter would
/// let someone fake a name, and the engine font draws them as boxes.
pub fn normalize(name: &str) -> Result<String, UsernameError> {
    let name = letters(name)?;

    if name.len() < MIN_LENGTH {
        return Err(UsernameError::TooShort);
    }
    Ok(name)
}

/// The start of a username, what somebody types into the search. The same
/// letters as a whole name, but one is enough, a search works from the first
/// key. `None` when nothing is typed.
pub fn normalize_start(start: &str) -> Result<Option<String>, UsernameError> {
    let start = letters(start)?;
    Ok((!start.is_empty()).then_some(start))
}

/// Trimmed and in lower case, or why the text cannot be a part of a username.
fn letters(text: &str) -> Result<String, UsernameError> {
    let text = text.trim();

    if !text
        .chars()
        .all(|letter| letter.is_ascii_alphanumeric() || letter == '_')
    {
        return Err(UsernameError::BadCharacter);
    }
    if text.len() > MAX_LENGTH {
        return Err(UsernameError::TooLong);
    }

    Ok(text.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::{UsernameError, normalize, normalize_start};

    #[test]
    fn a_search_works_from_the_first_letter() {
        assert_eq!(normalize_start("V").unwrap().as_deref(), Some("v"));
        assert_eq!(
            normalize_start(" iron_ ").unwrap().as_deref(),
            Some("iron_")
        );
        assert_eq!(normalize_start("  "), Ok(None));
        assert_eq!(
            normalize_start("two words"),
            Err(UsernameError::BadCharacter)
        );
        assert_eq!(
            normalize_start(&"a".repeat(21)),
            Err(UsernameError::TooLong)
        );
    }

    #[test]
    fn good_names_come_back_in_lower_case() {
        assert_eq!(normalize("Vladas").as_deref(), Ok("vladas"));
        assert_eq!(normalize("  iron_man_42 ").as_deref(), Ok("iron_man_42"));
        assert_eq!(normalize("abc").as_deref(), Ok("abc"));
        assert_eq!(normalize(&"a".repeat(20)), Ok("a".repeat(20)));
    }

    #[test]
    fn length_limits() {
        assert_eq!(normalize("ab"), Err(UsernameError::TooShort));
        assert_eq!(normalize(""), Err(UsernameError::TooShort));
        assert_eq!(normalize(&"a".repeat(21)), Err(UsernameError::TooLong));
    }

    #[test]
    fn other_characters_are_refused() {
        assert_eq!(normalize("two words"), Err(UsernameError::BadCharacter));
        assert_eq!(normalize("dash-name"), Err(UsernameError::BadCharacter));
        // A Cyrillic `а` in a Latin looking name.
        assert_eq!(normalize("vl\u{430}das"), Err(UsernameError::BadCharacter));
    }
}
