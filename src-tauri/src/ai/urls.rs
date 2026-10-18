//! Link handling that doesn't trust the model: URLs are found deterministically in the
//! note and OCR text, and a URL the model reports is kept only if it really appears there.

const TLDS: &[&str] = &[
    "com", "org", "net", "io", "in", "dev", "co", "ai", "app", "gov", "edu", "me", "xyz", "info", "tv", "gg", "so",
    "ly",
];

/// Web addresses in `text`: anything starting with http(s):// or www., or a bare domain with
/// a common TLD ("excalidraw.com", "youtube.com/watch?v=…").
pub fn extract(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for token in text.split_whitespace() {
        let t = token.trim_matches(|c: char| "()[]{}<>\"'“”‘’,;!".contains(c)).trim_end_matches(['.', ':', '?']);
        if looks_like_url(t) && !out.iter().any(|u| u.eq_ignore_ascii_case(t)) {
            out.push(t.to_owned());
        }
    }
    out
}

fn looks_like_url(t: &str) -> bool {
    let lower = t.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return lower.len() > "https://x.y".len() - 1 && lower.contains('.');
    }
    if lower.starts_with("www.") {
        return lower.len() > 6;
    }
    // Bare domain: labels of [a-z0-9-], a known TLD, then optionally a path.
    let host = lower.split(['/', '?', '#']).next().unwrap_or("");
    if host.contains('@') || !host.contains('.') {
        return false;
    }
    let labels: Vec<&str> = host.split('.').collect();
    let tld = labels.last().copied().unwrap_or("");
    labels.iter().all(|l| !l.is_empty() && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        && TLDS.contains(&tld)
}

/// The final link list: everything found in the source, plus any model-reported URL that
/// appears verbatim in the source. Invented URLs are dropped.
pub fn grounded(model_urls: &[String], source: &str) -> Vec<String> {
    let lower_source = source.to_lowercase();
    let mut out = extract(source);
    for u in model_urls {
        let u = u.trim();
        if !u.is_empty() && lower_source.contains(&u.to_lowercase()) && !out.iter().any(|x| x.eq_ignore_ascii_case(u)) {
            out.push(u.to_owned());
        }
    }
    out.truncate(10);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_urls_and_bare_domains() {
        let text = "Apply at https://hack.example.org/apply. Watch: youtube.com/watch?v=abc123XYZ \
                    (also www.figma.com) excalidraw.com, mail me@x.com, version 1.2.3, file.txt";
        assert_eq!(
            extract(text),
            ["https://hack.example.org/apply", "youtube.com/watch?v=abc123XYZ", "www.figma.com", "excalidraw.com"]
        );
    }

    #[test]
    fn model_urls_must_appear_in_the_source() {
        let source = "Visit Example.org/docs for more";
        let model = vec!["example.org/docs".to_string(), "https://made-up.example.com".to_string()];
        assert_eq!(grounded(&model, source), ["Example.org/docs"]);
    }

    #[test]
    fn no_duplicates() {
        assert_eq!(extract("https://a.com https://a.com HTTPS://A.COM"), ["https://a.com"]);
    }
}
