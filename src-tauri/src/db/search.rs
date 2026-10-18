//! Full-text search query building (FTS5 over title, summary, note, OCR text and tags).
//! The search itself runs inside the feed query (`db::feed`), ranked with BM25.

/// Turns what the user typed into a safe FTS5 query: every word must match, each as a
/// prefix ("hack" finds "hackathon"). FTS syntax characters can't be injected because
/// each word is quoted. `None` if there's nothing searchable.
pub fn fts_query(input: &str) -> Option<String> {
    let words: Vec<String> = input
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .take(12)
        .map(|w| format!("\"{}\"*", w.to_lowercase()))
        .collect();
    (!words.is_empty()).then(|| words.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_is_safe_and_prefix_based() {
        assert_eq!(fts_query("Hack dead"), Some(r#""hack"* "dead"*"#.into()));
        assert_eq!(fts_query(r#"a" OR title:x NEAR(*)"#), Some(r#""a"* "or"* "title"* "x"* "near"*"#.into()));
        assert_eq!(fts_query("  ... !!"), None);
    }
}
