//! Package ids made readable. `MaxGerman-Skip_Intro_Video` reads as the title
//! `Skip Intro Video` with `by MaxGerman` under it.

use blackforge_core::ident::PackageId;

/// The package name with spaces, or the text as it is when it is no id.
pub fn title(id: &str) -> String {
    id.parse::<PackageId>()
        .map_or_else(|_| id.to_owned(), |id| id.name().replace('_', " "))
}

/// `by MaxGerman`, empty when the text is no id.
pub fn author(id: &str) -> String {
    id.parse::<PackageId>()
        .map(|id| format!("by {}", id.owner()))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{author, title};

    #[test]
    fn reads_an_id() {
        assert_eq!(title("MaxGerman-Skip_Intro_Video"), "Skip Intro Video");
        assert_eq!(author("MaxGerman-Skip_Intro_Video"), "by MaxGerman");
        assert_eq!(title("LVH-IT-UsefulPaths"), "UsefulPaths");
        assert_eq!(author("LVH-IT-UsefulPaths"), "by LVH-IT");
        assert_eq!(title("not an id"), "not an id");
        assert_eq!(author("not an id"), "");
    }
}
