//! The home feed: every screenshot, newest first, narrowed by filters.
//!
//! Selected categories and tags must ALL match (a card needs every one). Text search is
//! separate (the header box) and combines with the filters; when it's used, results are
//! ordered by relevance instead of date.

use rusqlite::types::Value as SqlValue;
use rusqlite::{params_from_iter, Connection};
use serde::{Deserialize, Serialize};

use super::screenshots::{card_from_row, Card, CARD_COLUMNS};
use super::search::fts_query;
use crate::error::Result;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FeedQuery {
    pub category_ids: Vec<i64>,
    pub tags: Vec<String>,
    pub text: String,
}

/// FROM + WHERE for a query, with its parameters. `text` switches to an FTS join.
fn filter_sql(q: &FeedQuery) -> (String, Vec<SqlValue>, bool) {
    let mut params: Vec<SqlValue> = Vec::new();
    let fts = fts_query(&q.text);
    let mut sql = if fts.is_some() {
        "FROM screenshots_fts f JOIN screenshots s ON s.id = f.rowid JOIN categories c ON c.id = s.category_id
         WHERE screenshots_fts MATCH ? AND s.deleted_at IS NULL"
            .to_owned()
    } else {
        "FROM screenshots s JOIN categories c ON c.id = s.category_id WHERE s.deleted_at IS NULL".to_owned()
    };
    if let Some(m) = &fts {
        params.push(SqlValue::Text(m.clone()));
    }

    let mut categories = q.category_ids.clone();
    categories.sort_unstable();
    categories.dedup();
    if !categories.is_empty() {
        sql.push_str(
            " AND s.id IN (SELECT screenshot_id FROM screenshot_categories
                           WHERE category_id IN (SELECT value FROM json_each(?))
                           GROUP BY screenshot_id HAVING count(*) = ?)",
        );
        params.push(SqlValue::Text(serde_json::to_string(&categories).expect("ids serialize")));
        params.push(SqlValue::Integer(categories.len() as i64));
    }

    let mut tags: Vec<String> = q.tags.iter().map(|t| t.trim().to_lowercase()).filter(|t| !t.is_empty()).collect();
    tags.sort();
    tags.dedup();
    if !tags.is_empty() {
        sql.push_str(
            " AND s.id IN (SELECT screenshot_id FROM screenshot_tags
                           WHERE tag IN (SELECT value FROM json_each(?))
                           GROUP BY screenshot_id HAVING count(*) = ?)",
        );
        params.push(SqlValue::Text(serde_json::to_string(&tags).expect("tags serialize")));
        params.push(SqlValue::Integer(tags.len() as i64));
    }
    (sql, params, fts.is_some())
}

pub fn feed(conn: &Connection, q: &FeedQuery, limit: i64, offset: i64) -> Result<Vec<Card>> {
    let (from, mut params, ranked) = filter_sql(q);
    let order = if ranked {
        "ORDER BY bm25(screenshots_fts, 6.0, 3.0, 5.0, 1.0, 3.0), s.created_at DESC"
    } else {
        "ORDER BY s.created_at DESC, s.id DESC"
    };
    params.push(SqlValue::Integer(limit));
    params.push(SqlValue::Integer(offset));
    let mut stmt = conn.prepare_cached(&format!("SELECT {CARD_COLUMNS} {from} {order} LIMIT ? OFFSET ?"))?;
    let rows = stmt.query_map(params_from_iter(params), card_from_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn count(conn: &Connection, q: &FeedQuery) -> Result<i64> {
    let (from, params, _) = filter_sql(q);
    Ok(conn.prepare_cached(&format!("SELECT count(*) {from}"))?.query_row(params_from_iter(params), |r| r.get(0))?)
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TagCount {
    pub tag: String,
    pub count: i64,
}

/// Every tag in use, most used first (for the pill bar and the filter search).
pub fn tag_counts(conn: &Connection) -> Result<Vec<TagCount>> {
    let mut stmt = conn.prepare_cached(
        "SELECT t.tag, count(*) AS n FROM screenshot_tags t JOIN screenshots s ON s.id = t.screenshot_id
         WHERE s.deleted_at IS NULL GROUP BY t.tag ORDER BY n DESC, t.tag",
    )?;
    let rows = stmt.query_map([], |r| Ok(TagCount { tag: r.get(0)?, count: r.get(1)? }))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Tags on the screenshots that are in every one of `category_ids`, most used first:
/// the tag row follows the selected categories. No categories means every tag.
pub fn tag_counts_in(conn: &Connection, category_ids: &[i64]) -> Result<Vec<TagCount>> {
    if category_ids.is_empty() {
        return tag_counts(conn);
    }
    let (from_where, params, _) = filter_sql(&FeedQuery { category_ids: category_ids.to_vec(), ..Default::default() });
    let sql = format!(
        "SELECT t.tag, count(*) AS n FROM screenshot_tags t WHERE t.screenshot_id IN (SELECT s.id {from_where})
         GROUP BY t.tag ORDER BY n DESC, t.tag"
    );
    let mut stmt = conn.prepare_cached(&sql)?;
    let rows = stmt.query_map(params_from_iter(params), |r| Ok(TagCount { tag: r.get(0)?, count: r.get(1)? }))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;
    use crate::db::screenshots::{set_categories, soft_delete};
    use std::time::Instant;

    fn add(conn: &Connection, title: &str, cats: &[i64], tags: &[&str], at: i64) -> i64 {
        conn.execute(
            "INSERT INTO screenshots (category_id, image_path, thumb_path, width, height, title, tags_json, created_at, updated_at)
             VALUES (?1, 'i.png', 't.webp', 1, 1, ?2, ?3, ?4, ?4)",
            (cats[0], title, serde_json::to_string(tags).unwrap(), at),
        )
        .unwrap();
        let id = conn.last_insert_rowid();
        set_categories(conn, id, cats, false, at).unwrap();
        id
    }

    fn ids(conn: &Connection, q: FeedQuery) -> Vec<i64> {
        feed(conn, &q, 100, 0).unwrap().into_iter().map(|c| c.id).collect()
    }

    #[test]
    fn newest_first_and_filters_must_all_match() {
        let conn = open_in_memory().unwrap();
        let a = add(&conn, "Font site", &[3, 1], &["fonts", "design"], 10);
        let b = add(&conn, "Palette", &[1], &["design"], 20);
        let c = add(&conn, "Python tip", &[2], &["code"], 30);

        assert_eq!(ids(&conn, FeedQuery::default()), [c, b, a]);
        assert_eq!(ids(&conn, FeedQuery { category_ids: vec![1], ..Default::default() }), [b, a]);
        assert_eq!(ids(&conn, FeedQuery { category_ids: vec![1, 3], ..Default::default() }), [a]); // both
        assert_eq!(ids(&conn, FeedQuery { tags: vec!["Design".into()], ..Default::default() }), [b, a]);
        assert_eq!(ids(&conn, FeedQuery { tags: vec!["design".into(), "fonts".into()], ..Default::default() }), [a]);
        assert_eq!(
            ids(&conn, FeedQuery { category_ids: vec![1], tags: vec!["code".into()], ..Default::default() }),
            Vec::<i64>::new()
        );
        // Text search combines with filters.
        assert_eq!(ids(&conn, FeedQuery { category_ids: vec![1], text: "pal".into(), ..Default::default() }), [b]);
        assert_eq!(count(&conn, &FeedQuery { category_ids: vec![1], ..Default::default() }).unwrap(), 2);
        // Paging.
        assert_eq!(feed(&conn, &FeedQuery::default(), 1, 1).unwrap()[0].id, b);
        // Trash is hidden everywhere.
        soft_delete(&conn, b, 1).unwrap();
        assert_eq!(ids(&conn, FeedQuery { tags: vec!["design".into()], ..Default::default() }), [a]);
    }

    #[test]
    fn tag_counts_rank_by_use() {
        let conn = open_in_memory().unwrap();
        add(&conn, "a", &[1], &["design", "fonts"], 1);
        add(&conn, "b", &[1], &["Design"], 2);
        assert_eq!(
            tag_counts(&conn).unwrap(),
            [TagCount { tag: "design".into(), count: 2 }, TagCount { tag: "fonts".into(), count: 1 }]
        );
    }

    #[test]
    fn tag_counts_follow_the_selected_categories() {
        let conn = open_in_memory().unwrap();
        add(&conn, "a", &[1, 2], &["design", "fonts"], 1);
        add(&conn, "b", &[1], &["design", "rust"], 2);
        add(&conn, "c", &[2], &["fonts"], 3);
        let tags =
            |ids: &[i64]| tag_counts_in(&conn, ids).unwrap().into_iter().map(|t| (t.tag, t.count)).collect::<Vec<_>>();
        assert_eq!(tags(&[1]), [("design".into(), 2), ("fonts".into(), 1), ("rust".into(), 1)]);
        assert_eq!(tags(&[2]), [("fonts".into(), 2), ("design".into(), 1)]);
        assert_eq!(tags(&[1, 2]), [("design".into(), 1), ("fonts".into(), 1)]);
        assert_eq!(tags(&[]).len(), 3);
    }

    #[test]
    fn many_distinct_tags_stay_fast() {
        // 10,000 screenshots, 5,000 different tags (3 per screenshot), as a heavy library
        // might accumulate after a few years.
        let conn = open_in_memory().unwrap();
        let tx = conn.unchecked_transaction().unwrap();
        for i in 0..10_000i64 {
            let tags = [format!("tag{}", i % 5000), format!("tag{}", (i * 7) % 5000), "design".to_string()];
            tx.execute(
                "INSERT INTO screenshots (category_id, image_path, thumb_path, width, height, title, tags_json, created_at, updated_at)
                 VALUES (1, 'i.png', 't.webp', 1, 1, 'x', ?1, ?2, ?2)",
                (serde_json::to_string(&tags).unwrap(), i),
            )
            .unwrap();
            tx.execute("INSERT INTO screenshot_categories VALUES (?1, 1)", [tx.last_insert_rowid()]).unwrap();
        }
        tx.commit().unwrap();

        let t = Instant::now();
        let counts = tag_counts(&conn).unwrap();
        let counts_time = t.elapsed();
        assert_eq!(counts.len(), 5001);
        assert_eq!(counts[0].tag, "design");
        let json_bytes = serde_json::to_string(&counts).unwrap().len();

        let t = Instant::now();
        let hits =
            feed(&conn, &FeedQuery { tags: vec!["tag1234".into(), "design".into()], ..Default::default() }, 60, 0)
                .unwrap();
        let filter_time = t.elapsed();
        assert!(!hits.is_empty());
        assert!(counts_time.as_millis() < 500 && filter_time.as_millis() < 500);
        println!(
            "5,000 tags: tag_counts {counts_time:?} ({json_bytes} bytes to the UI), filter by rare tag {filter_time:?}"
        );
    }

    #[test]
    fn stays_fast_with_ten_thousand_items() {
        let conn = open_in_memory().unwrap();
        let tags = ["design", "fonts", "python", "palette", "career", "video", "recipe", "hackathon"];
        let tx = conn.unchecked_transaction().unwrap();
        for i in 0..10_000i64 {
            let t = [tags[(i % 8) as usize], tags[((i * 3 + 1) % 8) as usize]];
            tx.execute(
                "INSERT INTO screenshots (category_id, image_path, thumb_path, width, height, title, summary, tags_json, created_at, updated_at)
                 VALUES (?1, 'i.png', 't.webp', 1, 1, ?2, ?3, ?4, ?5, ?5)",
                (1 + i % 5, format!("{} item {i}", t[0]), format!("about {} and {}", t[0], t[1]), serde_json::to_string(&t).unwrap(), i),
            )
            .unwrap();
            let id = tx.last_insert_rowid();
            tx.execute("INSERT INTO screenshot_categories VALUES (?1, ?2)", (id, 1 + i % 5)).unwrap();
            if i % 2 == 0 {
                tx.execute("INSERT OR IGNORE INTO screenshot_categories VALUES (?1, 2)", [id]).unwrap();
            }
        }
        tx.commit().unwrap();

        let time = |q: FeedQuery| {
            let t = Instant::now();
            let n = feed(&conn, &q, 60, 0).unwrap().len();
            (n, t.elapsed())
        };
        let (n1, all) = time(FeedQuery::default());
        let (n2, cats) = time(FeedQuery { category_ids: vec![1, 2], ..Default::default() });
        let (n3, mixed) = time(FeedQuery {
            category_ids: vec![2],
            tags: vec!["design".into(), "fonts".into()],
            text: "about".into(),
        });
        assert_eq!((n1, n2), (60, 60));
        assert!(n3 > 0);
        for d in [all, cats, mixed] {
            assert!(d.as_millis() < 500, "feed query took {d:?}");
        }
        println!("10k feed: all {all:?}, 2 categories {cats:?}, category+2 tags+text {mixed:?}");
    }
}
