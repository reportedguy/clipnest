//! Deadline dates, resolved in Rust rather than trusted from the model (spec 3.6).
//!
//! The text the model quoted (`raw`) is parsed first with fixed rules: numeric dates are
//! DD/MM[/YYYY] (India), month names in either order, and ISO dates. Only if that fails
//! is the model's ISO date used, and only if it's a real calendar date. Missing years
//! become the next future occurrence; dates long past are ignored.

use std::sync::LazyLock;

use chrono::{Datelike, Duration, Local, NaiveDate, NaiveTime, TimeZone};
use regex::Regex;

/// Older than this (relative to when the screenshot was taken) isn't a deadline anymore.
const IGNORE_OLDER_THAN_DAYS: i64 = 30;
/// Further ahead than this is almost certainly a misread.
const IGNORE_LATER_THAN_DAYS: i64 = 3 * 366;

const MONTHS: &[&str] = &["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];

static ISO: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(\d{4})-(\d{1,2})-(\d{1,2})\b").unwrap());
static NUMERIC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(\d{1,2})[/.\-](\d{1,2})(?:[/.\-](\d{4}|\d{2}))?\b").unwrap());
static DAY_MONTH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(\d{1,2})(?:st|nd|rd|th)?\s+(?:of\s+)?([a-z]{3,9})\.?,?(?:\s+(\d{4}))?\b").unwrap()
});
static MONTH_DAY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b([a-z]{3,9})\.?\s+(\d{1,2})(?:st|nd|rd|th)?,?(?:\s+(\d{4}))?\b").unwrap());

fn month_number(word: &str) -> Option<u32> {
    let w = word.to_ascii_lowercase();
    // Full names or 3-letter abbreviations only ("mar", "march"; not "marketing").
    MONTHS
        .iter()
        .position(|m| {
            w.starts_with(m) && (w.len() == 3 || full_month(m).is_some_and(|f| f == w) || (*m == "sep" && w == "sept"))
        })
        .map(|i| i as u32 + 1)
}

fn full_month(abbr: &str) -> Option<&'static str> {
    const FULL: &[&str] = &[
        "january",
        "february",
        "march",
        "april",
        "may",
        "june",
        "july",
        "august",
        "september",
        "october",
        "november",
        "december",
    ];
    MONTHS.iter().position(|m| *m == abbr).map(|i| FULL[i])
}

/// The next occurrence of day/month on or after `reference` when no year was given.
fn with_year(day: u32, month: u32, year: Option<i32>, reference: NaiveDate) -> Option<NaiveDate> {
    match year {
        Some(y) => NaiveDate::from_ymd_opt(y, month, day),
        None => {
            let this_year = NaiveDate::from_ymd_opt(reference.year(), month, day)?;
            if this_year >= reference {
                Some(this_year)
            } else {
                NaiveDate::from_ymd_opt(reference.year() + 1, month, day)
            }
        }
    }
}

fn year(s: Option<regex::Match>) -> Option<i32> {
    s.and_then(|m| m.as_str().parse::<i32>().ok()).map(|y| if y < 100 { 2000 + y } else { y })
}

/// Parses the first date in free text, India-style (DD/MM).
pub fn parse_text(text: &str, reference: NaiveDate) -> Option<NaiveDate> {
    // Same OCR split-digit repair as the OCR stage, in case the text came from elsewhere.
    let repaired = crate::ocr::fix_split_numbers(text);
    let text = repaired.as_str();
    if let Some(c) = ISO.captures(text) {
        return NaiveDate::from_ymd_opt(c[1].parse().ok()?, c[2].parse().ok()?, c[3].parse().ok()?);
    }
    if let Some(c) = NUMERIC.captures(text) {
        let (day, month): (u32, u32) = (c[1].parse().ok()?, c[2].parse().ok()?);
        if let Some(d) = with_year(day, month, year(c.get(3)), reference) {
            return Some(d);
        }
    }
    for c in DAY_MONTH.captures_iter(text) {
        if let (Some(month), Ok(day)) = (month_number(&c[2]), c[1].parse()) {
            if let Some(d) = with_year(day, month, year(c.get(3)), reference) {
                return Some(d);
            }
        }
    }
    for c in MONTH_DAY.captures_iter(text) {
        if let (Some(month), Ok(day)) = (month_number(&c[1]), c[2].parse()) {
            if let Some(d) = with_year(day, month, year(c.get(3)), reference) {
                return Some(d);
            }
        }
    }
    None
}

/// The deadline date from the model's output: our own parse of the quoted text first,
/// then the model's ISO date. `None` if neither gives a plausible date.
pub fn resolve(raw: Option<&str>, iso: Option<&str>, reference: NaiveDate) -> Option<NaiveDate> {
    let ours = raw.and_then(|r| parse_text(r, reference));
    let theirs = iso.and_then(|s| NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok());
    ours.or(theirs).filter(|d| {
        let days = (*d - reference).num_days();
        (-IGNORE_OLDER_THAN_DAYS..=IGNORE_LATER_THAN_DAYS).contains(&days)
    })
}

/// End of that day (23:59) in local time, as epoch milliseconds.
pub fn end_of_day_ms(date: NaiveDate) -> i64 {
    let local = date.and_time(NaiveTime::from_hms_opt(23, 59, 0).expect("valid time"));
    Local
        .from_local_datetime(&local)
        .earliest()
        .map_or_else(|| local.and_utc().timestamp_millis(), |t| t.timestamp_millis())
}

/// When to remind: on the day `lead_hours` before the deadline, at `notify_hour` local.
pub fn remind_at_ms(deadline_ms: i64, lead_hours: u32, notify_hour: u8) -> i64 {
    let Some(deadline) = Local.timestamp_millis_opt(deadline_ms).single() else { return deadline_ms };
    let day = (deadline - Duration::hours(i64::from(lead_hours))).date_naive();
    let at = day.and_time(NaiveTime::from_hms_opt(u32::from(notify_hour.min(23)), 0, 0).expect("valid time"));
    Local.from_local_datetime(&at).earliest().map_or(deadline_ms, |t| t.timestamp_millis())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }
    const TODAY: (i32, u32, u32) = (2026, 10, 15);
    fn today() -> NaiveDate {
        d(TODAY.0, TODAY.1, TODAY.2)
    }

    #[test]
    fn numeric_dates_are_day_first() {
        assert_eq!(parse_text("Registrations close on 12/11/2026.", today()), Some(d(2026, 11, 12)));
        assert_eq!(parse_text("due 05-01-27", today()), Some(d(2027, 1, 5)));
        assert_eq!(parse_text("by 3.12", today()), Some(d(2026, 12, 3)));
        assert_eq!(parse_text("31/02/2027", today()), None); // not a real date
    }

    #[test]
    fn ocr_split_digits_do_not_change_the_date() {
        assert_eq!(parse_text("1 2/11/2026", today()), Some(d(2026, 11, 12)));
    }

    #[test]
    fn month_names_both_orders() {
        assert_eq!(parse_text("closes on 28 March 2027", today()), Some(d(2027, 3, 28)));
        assert_eq!(parse_text("Submissions due 5 Jan", today()), Some(d(2027, 1, 5))); // next occurrence
        assert_eq!(parse_text("Nov 20th, 2026", today()), Some(d(2026, 11, 20)));
        assert_eq!(parse_text("on the 1st of Sept", today()), Some(d(2027, 9, 1)));
        assert_eq!(parse_text("4 marketing tips", today()), None); // "marketing" isn't March
        assert_eq!(parse_text("2026-11-12", today()), Some(d(2026, 11, 12)));
    }

    #[test]
    fn missing_year_today_or_later_stays_this_year() {
        assert_eq!(parse_text("15 Oct", today()), Some(d(2026, 10, 15)));
        assert_eq!(parse_text("14 Oct", today()), Some(d(2027, 10, 14)));
    }

    #[test]
    fn resolve_prefers_our_parse_and_validates_the_model() {
        // The model got DD/MM backwards; our parse wins.
        assert_eq!(resolve(Some("12/11/2026"), Some("2026-12-11"), today()), Some(d(2026, 11, 12)));
        // Nothing parseable in raw: fall back to a valid ISO date from the model.
        assert_eq!(resolve(Some("next Friday"), Some("2026-10-23"), today()), Some(d(2026, 10, 23)));
        // Garbage from the model (seen in the spike) is rejected.
        assert_eq!(resolve(Some("soon"), Some("0303-20-27"), today()), None);
        // Long past or absurdly far: ignored.
        assert_eq!(resolve(Some("1/1/2020"), None, today()), None);
        assert_eq!(resolve(Some("1/1/2040"), None, today()), None);
        // A few days overdue is still a deadline (shown as overdue).
        assert_eq!(resolve(Some("10/10/2026"), None, today()), Some(d(2026, 10, 10)));
    }

    #[test]
    fn reminders_fire_on_the_lead_day_at_the_chosen_hour() {
        let deadline = end_of_day_ms(d(2026, 11, 12));
        let remind = Local.timestamp_millis_opt(remind_at_ms(deadline, 24, 9)).unwrap();
        assert_eq!((remind.date_naive(), remind.time()), (d(2026, 11, 11), NaiveTime::from_hms_opt(9, 0, 0).unwrap()));
        let same_day = Local.timestamp_millis_opt(remind_at_ms(deadline, 1, 9)).unwrap();
        assert_eq!(same_day.date_naive(), d(2026, 11, 12));
        let week = Local.timestamp_millis_opt(remind_at_ms(deadline, 168, 18)).unwrap();
        assert_eq!((week.date_naive(), week.time().format("%H:%M").to_string()), (d(2026, 11, 5), "18:00".into()));
    }
}
