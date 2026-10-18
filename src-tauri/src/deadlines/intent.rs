//! Deterministic "does the note show intent to act?" check, backing up the model's
//! judgement. The word list lives in `config/intent-keywords.txt` so it's easy to extend.

use std::sync::LazyLock;

static KEYWORDS: LazyLock<Vec<String>> = LazyLock::new(|| {
    include_str!("../../../config/intent-keywords.txt")
        .lines()
        .map(|l| l.trim().to_lowercase())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect()
});

/// True if any keyword appears in the note as whole words ("apply" matches "apply now"
/// but not "applying"; "due" doesn't match "subdued").
pub fn note_shows_intent(note: &str) -> bool {
    let note = note.to_lowercase();
    KEYWORDS.iter().any(|k| contains_phrase(&note, k))
}

fn contains_phrase(haystack: &str, phrase: &str) -> bool {
    let is_word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric());
    haystack.match_indices(phrase).any(|(i, _)| {
        let before = haystack[..i].chars().next_back();
        let after = haystack[i + phrase.len()..].chars().next();
        !is_word(before) && !is_word(after)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keywords_load_and_match_whole_words() {
        assert!(KEYWORDS.len() > 10);
        assert!(note_shows_intent("register for this"));
        assert!(note_shows_intent("Watch later!"));
        assert!(note_shows_intent("remind me on friday"));
        assert!(note_shows_intent("must APPLY before it closes"));
        assert!(!note_shows_intent("video idea"));
        assert!(!note_shows_intent(""));
        assert!(!note_shows_intent("applying is hard")); // whole words only
        assert!(!note_shows_intent("a subdued palette"));
    }
}
