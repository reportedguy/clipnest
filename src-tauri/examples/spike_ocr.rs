//! Spike 3: Windows.Media.Ocr from Rust: languages, speed, and accuracy.
//!
//!   cargo run --release --example spike_ocr -- <image.png> [expected.txt] ...
//!
//! For each image (optionally paired with its ground-truth text) prints the cold and warm
//! recognition time and the character error rate (CER).

use std::time::Instant;

use clipnest_lib::platform::native_ocr;

fn main() {
    let ocr = native_ocr();
    println!("installed OCR languages: {:?}", ocr.languages());

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut it = args.iter().peekable();
    while let Some(img_path) = it.next() {
        let expected = it.next_if(|a| a.ends_with(".txt")).map(|p| std::fs::read_to_string(p).unwrap());
        let img = image::open(img_path).expect("open image").to_rgba8();

        let t = Instant::now();
        let first = ocr.recognize(&img);
        let cold = t.elapsed();
        let mut warm = Vec::new();
        for _ in 0..5 {
            let t = Instant::now();
            ocr.recognize(&img).unwrap();
            warm.push(t.elapsed());
        }
        warm.sort();

        println!("\n== {img_path} ({}x{})", img.width(), img.height());
        match first {
            Ok(r) => {
                println!("cold {cold:?}, warm median {:?}, language {:?}", warm[2], r.language);
                println!("--- recognized ---\n{}\n------------------", r.text);
                if let Some(exp) = expected {
                    println!("CER: {:.1}%", 100.0 * cer(&normalize(&exp), &normalize(&r.text)));
                    // The production pipeline: invert dark images, then upscale small ones.
                    let t = Instant::now();
                    let r2 = clipnest_lib::ocr::recognize(ocr.as_ref(), &img).unwrap();
                    println!(
                        "with ocr::prepare: CER {:.1}% in {:?}",
                        100.0 * cer(&normalize(&exp), &normalize(&r2.text)),
                        t.elapsed()
                    );
                }
            }
            Err(e) => println!("error: {e}"),
        }
    }
}

/// Collapses whitespace so line-wrapping differences don't count as errors.
fn normalize(s: &str) -> Vec<char> {
    s.split_whitespace().collect::<Vec<_>>().join(" ").chars().collect()
}

/// Character error rate: Levenshtein distance / reference length.
fn cer(reference: &[char], hypothesis: &[char]) -> f64 {
    let mut prev: Vec<usize> = (0..=hypothesis.len()).collect();
    for (i, rc) in reference.iter().enumerate() {
        let mut cur = vec![i + 1; hypothesis.len() + 1];
        for (j, hc) in hypothesis.iter().enumerate() {
            cur[j + 1] = (prev[j] + usize::from(rc != hc)).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        prev = cur;
    }
    prev[hypothesis.len()] as f64 / reference.len().max(1) as f64
}
