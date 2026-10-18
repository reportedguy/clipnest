//! How many categories can the AI handle? Sends the real filing prompt with a growing
//! category list to the real model and reports prompt size, time, and whether the right
//! category is still picked.
//!
//!   cargo run --release --example category_capacity -- <app data dir> <model id> [counts...]
//!   e.g. ... -- "%APPDATA%/com.reportedsnipping.desktop" qwen3.5-4b-q4km 6 20 40 60 80 120

use std::path::PathBuf;
use std::time::Instant;

use chrono::NaiveDate;
use clipnest_lib::ai::engine::{Engine, Launch};
use clipnest_lib::ai::prompt::{self, CategoryRef, Request};
use clipnest_lib::ai::registry::Registry;
use serde_json::json;

const DEFAULTS: &[(&str, &str)] = &[
    ("Ideas", "content ideas, things I might make or write"),
    ("Learning", "knowledge, explanations, tips, how-tos"),
    ("Resources", "tools, websites, libraries, references"),
    ("Opportunities & Deadlines", "competitions, applications, events, jobs, anything with a date to act on"),
    ("Watch Later", "videos, talks, courses, films to watch"),
];

/// Realistic user-made categories (projects, interests) with typical hints.
fn extra(i: usize) -> (String, String) {
    const TOPICS: &[&str] = &[
        "Portfolio redesign",
        "Client Apollo",
        "Wedding planning",
        "Home renovation",
        "Trip to Japan",
        "Fitness plan",
        "Recipes",
        "Book notes",
        "Startup idea",
        "Investing research",
        "Course: ML basics",
        "Job search",
        "Thesis",
        "Freelance invoices",
        "Gift ideas",
        "Car maintenance",
        "Garden",
        "Podcast prep",
        "YouTube channel",
        "Newsletter",
        "Side project: Hermes",
        "Team onboarding",
        "Fonts",
        "Color palettes",
        "UI inspiration",
        "Marketing copy",
        "Legal documents",
        "Medical records",
        "Taxes 2026",
        "Apartment hunt",
    ];
    let t = TOPICS[i % TOPICS.len()];
    let n = i / TOPICS.len();
    let name = if n == 0 { t.to_string() } else { format!("{t} {}", n + 1) };
    (name.clone(), format!("screenshots, notes and references for {}", name.to_lowercase()))
}

fn categories(total: usize) -> Vec<CategoryRef> {
    let mut v: Vec<CategoryRef> = DEFAULTS
        .iter()
        .enumerate()
        .map(|(i, (n, h))| CategoryRef { id: i as i64 + 1, name: (*n).into(), hint: Some((*h).into()) })
        .collect();
    for i in 0..total.saturating_sub(DEFAULTS.len() + 1) {
        let (name, hint) = extra(i);
        v.push(CategoryRef { id: 100 + i as i64, name, hint: Some(hint) });
    }
    v.push(CategoryRef { id: 99, name: "Miscellaneous".into(), hint: None });
    v
}

const OCR: &str = "Global Student Hackathon 2026\nRegistrations close on 12/11/2026. Apply at https://hack.example.org/apply\nPrize pool: Rs 5,00,000 for the top three teams. Teams of up to 4 people.\nThemes: climate, health, education and open data.";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let data_dir = PathBuf::from(&args[0]);
    let model_id = &args[1];
    let counts: Vec<usize> = args[2..].iter().map(|c| c.parse().unwrap()).collect();

    let registry = Registry::load();
    let model = registry.model(model_id).expect("known model id");
    let d = model.download();
    let model_path = data_dir.join("ai/models").join(&d.id).join(d.file_name());
    let exe = data_dir.join("ai/engine").join(&registry.engine.id).join(registry.engine.binary.as_deref().unwrap());
    let log = std::env::temp_dir().join("rs-capacity-llama.log");
    let mut engine =
        Engine::start(&Launch { exe: &exe, model: &model_path, mmproj: None, log: &log }).expect("engine starts");
    let kwargs = model.chat_template_kwargs.clone().unwrap_or(json!({}));

    // Worst case: the longest screenshot text the app ever sends.
    let long_ocr: String = {
        let mut s = OCR.to_string();
        while s.chars().count() < prompt::MAX_OCR_CHARS {
            s.push_str(
                "\nMore event details: schedule, venue, judging criteria, mentors, FAQ and rules for participants.",
            );
        }
        s.chars().take(prompt::MAX_OCR_CHARS).collect()
    };

    println!("model {model_id}; context 4096 tokens");
    println!("{:>5}  {:>8}  {:>12}  {:>8}  {:<10}  answer", "cats", "ocr", "prompt toks", "seconds", "result");
    for &n in &counts {
        let cats = categories(n);
        for (label, ocr) in [("normal", OCR), ("longest", long_ocr.as_str())] {
            let req = Request {
                note: "register for this",
                ocr_text: ocr,
                categories: &cats,
                today: NaiveDate::from_ymd_opt(2026, 10, 16).unwrap(),
                timezone: "UTC+05:30",
            };
            let body = json!({
                "messages": prompt::messages(&req, false, None),
                "temperature": 0.2,
                "max_tokens": 400,
                "response_format": { "type": "json_schema", "json_schema": { "name": "screenshot", "schema": prompt::schema(&cats) } },
                "chat_template_kwargs": kwargs,
            });
            let t = Instant::now();
            match engine.chat(&body) {
                Ok(r) => {
                    let toks =
                        r["timings"]["prompt_n"].as_u64().unwrap_or(0) + r["timings"]["cache_n"].as_u64().unwrap_or(0);
                    let content = r["choices"][0]["message"]["content"].as_str().unwrap_or("");
                    let (result, answer) = match prompt::parse(content, &cats, "Miscellaneous") {
                        Ok(f) => {
                            let ok = f.categories.first().map(String::as_str) == Some("Opportunities & Deadlines");
                            (if ok { "correct" } else { "WRONG" }, format!("{:?} | {}", f.categories, f.title))
                        }
                        Err(e) => ("BAD JSON", e.to_string()),
                    };
                    println!(
                        "{n:>5}  {label:>8}  {toks:>12}  {:>8.1}  {result:<10}  {answer}",
                        t.elapsed().as_secs_f64()
                    );
                }
                Err(e) => println!(
                    "{n:>5}  {label:>8}  {:>12}  {:>8.1}  {:<10}  {e}",
                    "-",
                    t.elapsed().as_secs_f64(),
                    "FAILED"
                ),
            }
        }
    }
}
