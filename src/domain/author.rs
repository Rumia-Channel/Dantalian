use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Author {
    pub id: i64,
    pub ndl_id: Option<String>,
    pub name: String,
    pub transcription: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct CreateAuthor {
    pub name: String,
    pub transcription: Option<String>,
    pub ndl_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct UpdateAuthor {
    pub name: String,
    pub transcription: Option<String>,
    pub ndl_id: Option<String>,
}

/// Identity key for author matching across sources (NDL, tags, Discogs,
/// manual input). Display strings stay untouched; only comparisons use this.
/// Folds full-width ASCII, drops every whitespace run (half/full-width alike,
/// following the NDL lookup's space-stripped variants), and case-folds ASCII.
/// Deliberately NOT script conversion: "村上春樹" vs "Haruki Murakami" stay
/// distinct (genuinely ambiguous).
pub fn normalize_author_name(name: &str) -> String {
    name.chars()
        .filter_map(|ch| {
            if ch.is_whitespace() {
                None
            } else if ('\u{FF01}'..='\u{FF5E}').contains(&ch) {
                char::from_u32(ch as u32 - 0xFEE0)
            } else {
                Some(ch)
            }
        })
        .collect::<String>()
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::normalize_author_name;

    #[test]
    fn folds_spacing_case_and_fullwidth_for_matching() {
        assert_eq!(normalize_author_name("  村上　春樹  "), "村上春樹");
        assert_eq!(normalize_author_name("村上 春樹"), "村上春樹");
        assert_eq!(normalize_author_name("ＳＯＮＹ　Ｍｕｓｉｃ"), "sonymusic");
        assert_eq!(normalize_author_name("Sony Music"), "sonymusic");
    }

    #[test]
    fn keeps_scripts_distinct() {
        assert_ne!(
            normalize_author_name("村上春樹"),
            normalize_author_name("Haruki Murakami")
        );
    }
}
