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
    let name = name.trim();

    if !name
        .chars()
        .all(|letter| letter.is_ascii_alphanumeric() || letter == '_')
    {
        return Err(UsernameError::BadCharacter);
    }
    if name.len() < MIN_LENGTH {
        return Err(UsernameError::TooShort);
    }
    if name.len() > MAX_LENGTH {
        return Err(UsernameError::TooLong);
    }

    Ok(name.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::{UsernameError, normalize};

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
