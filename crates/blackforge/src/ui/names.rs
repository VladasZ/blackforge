//! Package ids made readable. `MaxGerman-Skip_Intro_Video` reads as the title
//! `Skip Intro Video` with `By MaxGerman` under it.

use blackforge_core::ident::PackageId;

/// The package name with spaces, or the text as it is when it is no id.
pub fn title(id: &str) -> String {
    id.parse::<PackageId>()
        .map_or_else(|_| id.to_owned(), |id| id.name().replace('_', " "))
}

/// `By MaxGerman`, empty when the text is no id.
pub fn author(id: &str) -> String {
    id.parse::<PackageId>()
        .map(|id| format!("By {}", id.owner()))
        .unwrap_or_default()
}

/// The text with its first letter in upper case, so a line from the core
/// like `data folder` reads as `Data folder`.
pub fn sentence(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

#[cfg(test)]
mod tests {
    use super::{author, sentence, title};

    #[test]
    fn starts_a_sentence() {
        assert_eq!(sentence("data folder"), "Data folder");
        assert_eq!(sentence("D:\\Games"), "D:\\Games");
        assert_eq!(sentence(""), "");
    }

    #[test]
    fn reads_an_id() {
        assert_eq!(title("MaxGerman-Skip_Intro_Video"), "Skip Intro Video");
        assert_eq!(author("MaxGerman-Skip_Intro_Video"), "By MaxGerman");
        assert_eq!(title("LVH-IT-UsefulPaths"), "UsefulPaths");
        assert_eq!(author("LVH-IT-UsefulPaths"), "By LVH-IT");
        assert_eq!(title("not an id"), "not an id");
        assert_eq!(author("not an id"), "");
    }
}
