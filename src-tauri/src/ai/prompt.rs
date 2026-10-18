//! Builds the filing request from versioned prompt files (`/prompts`) and validates the
//! model's answer. Pure functions, so they're fully unit-tested without a model.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::{Error, Result};

pub const PROMPT_VERSION: &str = "categorize.v3";
const SYSTEM: &str = include_str!("../../../prompts/categorize.v3.md");
const SCHEMA: &str = include_str!("../../../prompts/categorize.v3.schema.json");
const EXAMPLES: &str = include_str!("../../../prompts/categorize.v3.examples.json");
/// The AI may file a screenshot into at most this many categories.
pub const MAX_CATEGORIES: usize = 3;

/// Users are in India (spec 3.4): ambiguous numeric dates are DD/MM/YYYY.
pub const LOCALE: &str = "India (read numeric dates like 12/11/2026 as DD/MM/YYYY)";

/// OCR text beyond this is cut from the middle; the model's context is 4096 tokens and
/// the prompt, examples and answer need room too.
pub const MAX_OCR_CHARS: usize = 4000;
const TRUNCATION_MARK: &str = "\n[… the middle of this text was cut because it was long …]\n";

#[derive(Debug, Clone)]
pub struct CategoryRef {
    pub id: i64,
    pub name: String,
    pub hint: Option<String>,
}

/// Everything the model sees about one screenshot.
pub struct Request<'a> {
    pub note: &'a str,
    pub ocr_text: &'a str,
    pub categories: &'a [CategoryRef],
    pub today: NaiveDate,
    /// e.g. "UTC+05:30"
    pub timezone: &'a str,
}

/// The model's answer, as constrained by the JSON schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Filing {
    pub title: String,
    pub summary: String,
    /// 1-3 of the user's category names, best match first.
    pub categories: Vec<String>,
    /// What the note says the person wants ("" when the note is only an aside).
    #[serde(default)]
    pub note_purpose: String,
    pub tags: Vec<String>,
    pub urls: Vec<String>,
    pub note_has_action_intent: bool,
    pub deadline: Option<Deadline>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Deadline {
    pub raw: String,
    pub iso: Option<String>,
    pub label: String,
}

pub fn system_prompt(req: &Request, strict: bool) -> String {
    let mut s = SYSTEM
        .replace("{{locale}}", LOCALE)
        .replace("{{today}}", &req.today.format("%Y-%m-%d (%A)").to_string())
        .replace("{{timezone}}", req.timezone);
    if strict {
        s.push_str(
            "\n\nIMPORTANT: your previous reply could not be used. Reply with one JSON object only, exactly matching the \
             schema, with \"category\" copied exactly from the category list.",
        );
    }
    s
}

/// The JSON schema with categories restricted to the user's own category names, so the
/// model can't invent one.
pub fn schema(categories: &[CategoryRef]) -> Value {
    let mut schema: Value = serde_json::from_str(SCHEMA).expect("schema file is valid JSON (tested)");
    schema["properties"]["categories"]["items"]["enum"] = json!(categories.iter().map(|c| &c.name).collect::<Vec<_>>());
    schema
}

pub fn user_message(req: &Request) -> String {
    let categories = category_list(req.categories);
    let note = if req.note.trim().is_empty() { "(none)" } else { req.note.trim() };
    let (text, _) = truncate_middle(req.ocr_text.trim(), MAX_OCR_CHARS);
    let text = if text.is_empty() { "(no text found)".to_owned() } else { text };
    // The note goes last: small models weigh what they read last most, and the note is
    // what should decide the title when it states a purpose.
    format!(
        "Categories:\n{categories}\n\nScreenshot text:\n\"\"\"\n{text}\n\"\"\"\n\nThe person's note (most important): {note}"
    )
}

fn category_list(categories: &[CategoryRef]) -> String {
    categories
        .iter()
        .map(|c| match c.hint.as_deref().filter(|h| !h.trim().is_empty()) {
            Some(h) => format!("- {}: {}", c.name, h.trim()),
            None => format!("- {}", c.name),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Chat messages: system prompt, few-shot examples, then this screenshot. The prefix is
/// identical across requests, so llama-server's prompt cache only processes it once.
pub fn messages(req: &Request, strict: bool, image_data_url: Option<&str>) -> Vec<Value> {
    let mut out = vec![json!({ "role": "system", "content": system_prompt(req, strict) })];
    let examples: Vec<Value> = serde_json::from_str(EXAMPLES).expect("examples file is valid JSON (tested)");
    for ex in examples {
        out.push(json!({ "role": "user", "content": ex["user"] }));
        out.push(json!({ "role": "assistant", "content": ex["assistant"].to_string() }));
    }
    let text = user_message(req);
    out.push(match image_data_url {
        None => json!({ "role": "user", "content": text }),
        Some(url) => json!({ "role": "user", "content": [
            { "type": "text", "text": format!("{text}\n\nThe screenshot itself is attached: use it to see what this is about. The note still decides the purpose and the title.") },
            { "type": "image_url", "image_url": { "url": url } },
        ]}),
    });
    out
}

/// Keeps the start and end of long text, dropping the middle. Returns whether it cut.
pub fn truncate_middle(text: &str, max_chars: usize) -> (String, bool) {
    let count = text.chars().count();
    if count <= max_chars {
        return (text.to_owned(), false);
    }
    let head = max_chars * 3 / 5;
    let tail = max_chars - head;
    let start: String = text.chars().take(head).collect();
    let end: String = text.chars().skip(count - tail).collect();
    (format!("{start}{TRUNCATION_MARK}{end}"), true)
}

/// Parses and sanity-checks the model's reply. The category must be one of the user's
/// (matched case-insensitively, falling back to Miscellaneous if the model slipped).
pub fn parse(raw: &str, categories: &[CategoryRef], misc_name: &str) -> Result<Filing> {
    let raw = raw.trim().trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();
    let mut f: Filing =
        serde_json::from_str(raw).map_err(|e| Error::Invalid(format!("the AI's answer wasn't valid JSON ({e})")))?;

    f.title = clean_line(&f.title, 90);
    f.summary = clean_line(&f.summary, 320);
    if f.title.is_empty() {
        return Err(Error::Invalid("the AI's answer had no title".into()));
    }
    // Only the user's categories (matched case-insensitively), no repeats, at most
    // MAX_CATEGORIES, and Miscellaneous only when nothing else applies.
    let mut chosen: Vec<String> = Vec::new();
    for name in &f.categories {
        if let Some(c) = categories.iter().find(|c| c.name.eq_ignore_ascii_case(name.trim())) {
            if !chosen.contains(&c.name) {
                chosen.push(c.name.clone());
            }
        }
    }
    if chosen.len() > 1 {
        chosen.retain(|n| n != misc_name);
    }
    chosen.truncate(MAX_CATEGORIES);
    if chosen.is_empty() {
        chosen.push(misc_name.to_owned());
    }
    f.categories = chosen;

    let mut tags: Vec<String> = Vec::new();
    for t in &f.tags {
        let t = clean_line(&t.to_lowercase(), 24);
        if !t.is_empty() && !tags.contains(&t) {
            tags.push(t);
        }
    }
    tags.truncate(4);
    f.tags = tags;
    Ok(f)
}

fn clean_line(s: &str, max: usize) -> String {
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.chars().count() <= max {
        s
    } else {
        let mut cut: String = s.chars().take(max - 1).collect();
        cut.push('…');
        cut
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cats() -> Vec<CategoryRef> {
        vec![
            CategoryRef { id: 1, name: "Ideas".into(), hint: Some("content ideas".into()) },
            CategoryRef { id: 2, name: "Opportunities & Deadlines".into(), hint: None },
            CategoryRef { id: 6, name: "Miscellaneous".into(), hint: None },
        ]
    }

    fn req<'a>(note: &'a str, ocr: &'a str, categories: &'a [CategoryRef]) -> Request<'a> {
        Request {
            note,
            ocr_text: ocr,
            categories,
            today: NaiveDate::from_ymd_opt(2026, 10, 15).unwrap(),
            timezone: "UTC+05:30",
        }
    }

    #[test]
    fn prompt_files_are_valid_and_filled_in() {
        let c = cats();
        let r = req("register for this", "Hackathon closes 12/11/2026", &c);
        let sys = system_prompt(&r, false);
        assert!(sys.contains("2026-10-15 (Thursday)") && sys.contains("DD/MM/YYYY") && sys.contains("UTC+05:30"));
        assert!(!sys.contains("{{"));
        assert!(system_prompt(&r, true).contains("IMPORTANT"));
        let s = schema(&c);
        assert_eq!(
            s["properties"]["categories"]["items"]["enum"],
            json!(["Ideas", "Opportunities & Deadlines", "Miscellaneous"])
        );
        assert_eq!(s["properties"]["categories"]["maxItems"], json!(MAX_CATEGORIES));
        let m = messages(&r, false, None);
        assert_eq!(m.len(), 1 + 5 * 2 + 1); // system + 5 examples + this one
        let last = m.last().unwrap()["content"].as_str().unwrap();
        assert!(last.contains("- Ideas: content ideas\n- Opportunities & Deadlines\n- Miscellaneous"));
        assert!(last.ends_with("The person's note (most important): register for this"));
        // Examples are valid filings themselves.
        let examples: Vec<Value> = serde_json::from_str(EXAMPLES).unwrap();
        for ex in examples {
            serde_json::from_value::<Filing>(ex["assistant"].clone()).unwrap();
        }
    }

    #[test]
    fn empty_note_and_text_are_explicit() {
        let c = cats();
        let msg = user_message(&req("  ", "", &c));
        assert!(msg.ends_with("The person's note (most important): (none)") && msg.contains("(no text found)"));
    }

    #[test]
    fn image_requests_attach_the_picture() {
        let c = cats();
        let m = messages(&req("", "", &c), false, Some("data:image/png;base64,AAAA"));
        assert_eq!(m.last().unwrap()["content"][1]["image_url"]["url"], "data:image/png;base64,AAAA");
    }

    #[test]
    fn long_text_keeps_head_and_tail() {
        let text: String = (0..10_000).map(|i| char::from(b'a' + (i % 26) as u8)).collect();
        let (out, cut) = truncate_middle(&text, 1000);
        assert!(cut);
        assert!(out.starts_with(&text[..600]) && out.ends_with(&text[text.len() - 400..]));
        assert!(out.contains("middle of this text was cut"));
        assert_eq!(truncate_middle("short", 1000), ("short".into(), false));
        // Never splits a multi-byte character.
        let (hindi, _) = truncate_middle(&"नमस्ते ".repeat(1000), 100);
        assert!(hindi.is_char_boundary(hindi.len()));
    }

    #[test]
    fn parse_validates_and_normalizes() {
        let c = cats();
        let raw = r#"```json
        {"title":"  Hackathon   registration ","summary":"Register by 12 Nov.","categories":["opportunities & deadlines","Miscellaneous","Ideas","Ideas"],
         "tags":["Hackathon","hackathon","Students","Prize Money","extra"],"urls":["https://hack.example.org/apply"],
         "note_has_action_intent":true,"deadline":{"raw":"12/11/2026","iso":"2026-11-12","label":"registration closes"}}
        ```"#;
        let f = parse(raw, &c, "Miscellaneous").unwrap();
        assert_eq!(f.title, "Hackathon registration");
        // Canonical names, Misc dropped next to real ones, duplicates removed, order kept.
        assert_eq!(f.categories, ["Opportunities & Deadlines", "Ideas"]);
        assert_eq!(f.tags, ["hackathon", "students", "prize money", "extra"]);
        assert_eq!(f.deadline.unwrap().iso.as_deref(), Some("2026-11-12"));
    }

    #[test]
    fn parse_falls_back_and_rejects_garbage() {
        let c = cats();
        let unknown = r#"{"title":"T","summary":"S","categories":["Recipes"],"tags":[],"urls":[],"note_has_action_intent":false,"deadline":null}"#;
        assert_eq!(parse(unknown, &c, "Miscellaneous").unwrap().categories, ["Miscellaneous"]);
        let too_many = r#"{"title":"T","summary":"S","categories":["Ideas","Opportunities & Deadlines","Miscellaneous","Ideas"],"tags":[],"urls":[],"note_has_action_intent":false,"deadline":null}"#;
        assert_eq!(parse(too_many, &c, "Miscellaneous").unwrap().categories.len(), 2);
        assert!(parse("not json", &c, "Miscellaneous").is_err());
        let no_title = r#"{"title":"  ","summary":"S","categories":["Ideas"],"tags":[],"urls":[],"note_has_action_intent":false,"deadline":null}"#;
        assert!(parse(no_title, &c, "Miscellaneous").is_err());
    }
}
