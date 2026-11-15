//! Exact sizes of the groups a History list shows, answered from indexes.
//!
//! A count has to equal what paging the same query to its end would list, so
//! every predicate here is the list's predicate read from an index that
//! carries its column. A filter no index carries (text search, content class,
//! semantic kind) has no cheap exact count and yields `None`, never an
//! estimate: reading those columns walks each row's encrypted payload.

use copypaste_ipc::{
    HistoryCountRange, HistoryGroupCounts, HistoryQuery, MAX_HISTORY_COUNT_RANGES,
};
use rusqlite::{params_from_iter, types::Value};

use super::history::{push_tag_filter, validate_filters};
use super::model::StoreError;
use super::store::Store;

const PINNED_SQL: &str = "SELECT COUNT(*) FROM clipboard_items ci \
    INDEXED BY idx_items_history WHERE ci.deleted = 0 AND ci.pinned = 1";

/// `?1` and `?2` are the range, rebound for each group.
const RANGE_SQL: &str = "SELECT COUNT(*) FROM clipboard_items ci \
    INDEXED BY idx_items_evictable WHERE ci.deleted = 0 AND ci.pinned = 0 \
    AND ci.created_at >= ?1 AND ci.created_at < ?2";

/// `(created_at, id)` names the row in the sync index, which carries its
/// origin; the source-application index is sought by rowid.
const ORIGIN_SQL: &str = " AND EXISTS (SELECT 1 FROM clipboard_items o \
    INDEXED BY idx_items_syncable \
    WHERE o.created_at = ci.created_at AND o.id = ci.id AND o.origin_device_id ";
const SOURCE_APP_SQL: &str = " AND EXISTS (SELECT 1 FROM clipboard_items a \
    INDEXED BY idx_items_source_app \
    WHERE a.deleted = 0 AND a.rowid = ci.rowid AND a.app_bundle_id = ";

fn countable(query: &HistoryQuery) -> bool {
    query
        .search
        .as_deref()
        .is_none_or(|raw| raw.trim().is_empty())
        && query.content_classes.is_empty()
        && query.semantic_kinds.is_empty()
}

/// The list's date, origin and source-application predicates, index-only.
fn push_filters(
    sql: &mut String,
    values: &mut Vec<Value>,
    query: &HistoryQuery,
    local_device_id: Option<&str>,
) {
    if let Some(from) = query.captured_from_ms {
        values.push(Value::Integer(from));
        sql.push_str(&format!(" AND ci.created_at >= ?{}", values.len()));
    }
    if let Some(before) = query.captured_before_ms {
        values.push(Value::Integer(before));
        sql.push_str(&format!(" AND ci.created_at < ?{}", values.len()));
    }
    if let Some(origin) = &query.origin_device_id {
        values.push(Value::Text(origin.clone()));
        sql.push_str(ORIGIN_SQL);
        // A local capture stores an empty origin, as the list query knows.
        if Some(origin.as_str()) == local_device_id {
            sql.push_str(&format!("IN ('', ?{}))", values.len()));
        } else {
            sql.push_str(&format!("= ?{})", values.len()));
        }
    }
    if let Some(source) = &query.source_app_bundle_id {
        values.push(Value::Text(source.clone()));
        sql.push_str(SOURCE_APP_SQL);
        sql.push_str(&format!("?{})", values.len()));
    }
}

fn pinned_sql(query: &HistoryQuery, local_device_id: Option<&str>) -> (String, Vec<Value>) {
    let mut sql = String::from(PINNED_SQL);
    let mut values = Vec::new();
    push_filters(&mut sql, &mut values, query, local_device_id);
    push_tag_filter(&mut sql, &mut values, query);
    (sql, values)
}

fn range_sql(query: &HistoryQuery, local_device_id: Option<&str>) -> (String, Vec<Value>) {
    let mut sql = String::from(RANGE_SQL);
    let mut values = vec![Value::Null, Value::Null];
    push_filters(&mut sql, &mut values, query, local_device_id);
    (sql, values)
}

impl Store {
    /// Count the pinned clips `query` lists and its unpinned clips in each of
    /// `ranges`, in one read snapshot.
    ///
    /// The work is one covering index range per group plus one index seek per
    /// filtered row inside it; rows outside the requested groups are never
    /// visited. `None` means the query has a filter that cannot be counted
    /// this way. Rows that later fail to decrypt are counted: telling them
    /// apart needs the payload.
    pub fn history_group_counts(
        &self,
        query: &HistoryQuery,
        ranges: &[HistoryCountRange],
        local_device_id: Option<&str>,
    ) -> Result<Option<HistoryGroupCounts>, StoreError> {
        validate_filters(query)?;
        if ranges.len() > MAX_HISTORY_COUNT_RANGES
            || ranges.iter().any(|range| range.from_ms >= range.before_ms)
        {
            return Err(StoreError::InvalidHistoryQuery);
        }
        if !countable(query) {
            return Ok(None);
        }

        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let count = |sql: &str, values: &[Value]| -> rusqlite::Result<u64> {
            let total: i64 = tx
                .prepare_cached(sql)?
                .query_row(params_from_iter(values.iter()), |row| row.get(0))?;
            Ok(total.max(0) as u64)
        };
        let (sql, values) = pinned_sql(query, local_device_id);
        let pinned = count(&sql, &values)?;

        // Pin and tag filters list pinned clips only, so no range has rows.
        let unpinned_listed =
            !query.pinned_only && !query.untagged_only && query.tag_ids.is_empty();
        let mut counts = vec![0; ranges.len()];
        if unpinned_listed {
            let (sql, mut values) = range_sql(query, local_device_id);
            for (range, slot) in ranges.iter().zip(&mut counts) {
                values[0] = Value::Integer(range.from_ms);
                values[1] = Value::Integer(range.before_ms);
                *slot = count(&sql, &values)?;
            }
        }
        Ok(Some(HistoryGroupCounts {
            pinned,
            ranges: counts,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{item, plan_of, store, T0};
    use super::*;

    const DAY: i64 = 86_400_000;
    const LOCAL: &str = "local-device";

    fn day(index: i64) -> HistoryCountRange {
        HistoryCountRange {
            from_ms: T0 + index * DAY,
            before_ms: T0 + (index + 1) * DAY,
        }
    }

    fn counts(s: &Store, query: &HistoryQuery, ranges: &[HistoryCountRange]) -> HistoryGroupCounts {
        s.history_group_counts(query, ranges, Some(LOCAL))
            .unwrap()
            .expect("the query is countable")
    }

    /// What paging the list to its end shows, grouped the way a client does.
    fn listed(s: &Store, query: &HistoryQuery, ranges: &[HistoryCountRange]) -> HistoryGroupCounts {
        let mut shown = HistoryGroupCounts {
            pinned: 0,
            ranges: vec![0; ranges.len()],
        };
        let mut after = None;
        loop {
            let page = s
                .query_history_bounded_for_device(query, after.as_ref(), 2, usize::MAX, Some(LOCAL))
                .unwrap();
            for row in &page.items {
                if row.pinned {
                    shown.pinned += 1;
                    continue;
                }
                for (range, slot) in ranges.iter().zip(&mut shown.ranges) {
                    if (range.from_ms..range.before_ms).contains(&row.created_at) {
                        *slot += 1;
                    }
                }
            }
            match page.next {
                Some(next) => after = Some(next),
                None => return shown,
            }
        }
    }

    fn seeded() -> Store {
        let s = store();
        let set_origin = |id: &str, origin: &str| {
            s.conn()
                .unwrap()
                .execute(
                    "UPDATE clipboard_items SET origin_device_id = ?2 WHERE id = ?1",
                    rusqlite::params![id, origin],
                )
                .unwrap();
        };
        for n in 0..5 {
            let mut clip = item(&format!("today {n}"), T0 + 2 * DAY + n);
            if n % 2 == 0 {
                clip.app_bundle_id = Some("com.example.editor".into());
            }
            let stored = s.insert(clip).unwrap();
            match n {
                1 => set_origin(&stored.id, "remote-device"),
                2 => set_origin(&stored.id, LOCAL),
                _ => {}
            }
        }
        for n in 0..3 {
            let mut clip = item(&format!("yesterday {n}"), T0 + DAY + n);
            clip.app_bundle_id = Some("com.example.editor".into());
            let stored = s.insert(clip).unwrap();
            if n == 0 {
                set_origin(&stored.id, "remote-device");
            }
        }
        let old_pin = s.insert(item("pinned long ago", T0 + 7)).unwrap();
        let mut today_pin = item("pinned today", T0 + 2 * DAY + 9);
        today_pin.app_bundle_id = Some("com.example.editor".into());
        let today_pin = s.insert(today_pin).unwrap();
        for pin in [&old_pin, &today_pin] {
            assert!(s.set_pinned(&pin.id, true).unwrap());
        }
        let deleted = s.insert(item("deleted today", T0 + 2 * DAY + 20)).unwrap();
        assert!(s.delete(&deleted.id).unwrap());
        s
    }

    #[test]
    fn counts_equal_what_the_list_pages_through_for_every_countable_filter() {
        let s = seeded();
        let ranges = [day(2), day(1), day(0), day(5)];
        let unfiltered = counts(&s, &HistoryQuery::default(), &ranges);
        assert_eq!(
            unfiltered,
            HistoryGroupCounts {
                pinned: 2,
                ranges: vec![5, 3, 0, 0],
            }
        );

        for query in [
            HistoryQuery::default(),
            HistoryQuery {
                origin_device_id: Some(LOCAL.into()),
                ..HistoryQuery::default()
            },
            HistoryQuery {
                origin_device_id: Some("remote-device".into()),
                ..HistoryQuery::default()
            },
            HistoryQuery {
                source_app_bundle_id: Some("com.example.editor".into()),
                ..HistoryQuery::default()
            },
            HistoryQuery {
                origin_device_id: Some(LOCAL.into()),
                source_app_bundle_id: Some("com.example.editor".into()),
                captured_from_ms: Some(T0 + DAY + 1),
                captured_before_ms: Some(T0 + 2 * DAY + 3),
                ..HistoryQuery::default()
            },
            HistoryQuery {
                pinned_only: true,
                ..HistoryQuery::default()
            },
            HistoryQuery {
                untagged_only: true,
                source_app_bundle_id: Some("com.example.editor".into()),
                ..HistoryQuery::default()
            },
            HistoryQuery {
                sort: copypaste_ipc::HistorySort::Oldest,
                ..HistoryQuery::default()
            },
        ] {
            assert_eq!(
                counts(&s, &query, &ranges),
                listed(&s, &query, &ranges),
                "{query:?}"
            );
        }
    }

    /// The runtime knows no time zone. A client's local days around a clock
    /// change are 23 and 25 hours long (Europe/Kyiv, 2026), and only the
    /// instants it sends decide which day a clip is counted in.
    #[test]
    fn ranges_are_half_open_instants_so_short_and_long_days_partition() {
        const SPRING_DAY: (i64, i64) = (1_774_735_200_000, 1_774_818_000_000);
        const AUTUMN_DAY: (i64, i64) = (1_792_875_600_000, 1_792_965_600_000);
        assert_eq!(SPRING_DAY.1 - SPRING_DAY.0, 23 * 3_600_000);
        assert_eq!(AUTUMN_DAY.1 - AUTUMN_DAY.0, 25 * 3_600_000);

        let s = store();
        for (start, end) in [SPRING_DAY, AUTUMN_DAY] {
            for stamp in [start - 1, start, end - 1, end] {
                s.insert(item(&format!("at {stamp}"), stamp)).unwrap();
            }
        }
        let ranges = [SPRING_DAY, AUTUMN_DAY].map(|(start, end)| {
            [
                HistoryCountRange {
                    from_ms: start - DAY,
                    before_ms: start,
                },
                HistoryCountRange {
                    from_ms: start,
                    before_ms: end,
                },
                HistoryCountRange {
                    from_ms: end,
                    before_ms: end + DAY,
                },
            ]
        });
        let counted = counts(&s, &HistoryQuery::default(), &ranges.concat());
        assert_eq!(counted.ranges, vec![1, 2, 1, 1, 2, 1]);
    }

    #[test]
    fn counts_follow_deletes_pins_and_retention() {
        let s = store();
        let ids: Vec<String> = (0..6)
            .map(|n| s.insert(item(&format!("clip {n}"), T0 + n)).unwrap().id)
            .collect();
        let today = [day(0)];
        assert_eq!(counts(&s, &HistoryQuery::default(), &today).ranges, [6]);

        assert!(s.delete(&ids[5]).unwrap());
        assert!(s.set_pinned(&ids[4], true).unwrap());
        assert_eq!(
            counts(&s, &HistoryQuery::default(), &today),
            HistoryGroupCounts {
                pinned: 1,
                ranges: vec![4],
            }
        );

        assert_eq!(s.evict_over_cap(3).unwrap(), 2);
        assert_eq!(
            counts(&s, &HistoryQuery::default(), &today),
            HistoryGroupCounts {
                pinned: 1,
                ranges: vec![2],
            }
        );
    }

    #[test]
    fn a_filter_no_index_answers_is_reported_as_uncounted_not_estimated() {
        let s = seeded();
        for query in [
            HistoryQuery {
                search: Some("today".into()),
                ..HistoryQuery::default()
            },
            HistoryQuery {
                content_classes: vec![copypaste_ipc::ContentClass::Image],
                ..HistoryQuery::default()
            },
            HistoryQuery {
                semantic_kinds: vec![copypaste_ipc::SemanticKind::Link],
                ..HistoryQuery::default()
            },
        ] {
            assert_eq!(
                s.history_group_counts(&query, &[day(2)], Some(LOCAL))
                    .unwrap(),
                None,
                "{query:?}"
            );
        }
        let blank_search = HistoryQuery {
            search: Some("  ".into()),
            ..HistoryQuery::default()
        };
        assert_eq!(counts(&s, &blank_search, &[day(2)]).ranges, [5]);

        let empty = HistoryCountRange {
            from_ms: T0,
            before_ms: T0,
        };
        let too_many = vec![day(0); MAX_HISTORY_COUNT_RANGES + 1];
        for ranges in [&[empty][..], &too_many[..]] {
            assert!(matches!(
                s.history_group_counts(&HistoryQuery::default(), ranges, None),
                Err(StoreError::InvalidHistoryQuery)
            ));
        }
    }

    /// Every step must stay inside an index: a table read here walks the
    /// encrypted payload that precedes the filtered columns in each row.
    #[test]
    fn every_count_is_answered_from_covering_indexes() {
        let s = seeded();
        let query = HistoryQuery {
            origin_device_id: Some(LOCAL.into()),
            source_app_bundle_id: Some("com.example.editor".into()),
            captured_from_ms: Some(T0),
            captured_before_ms: Some(T0 + 9 * DAY),
            ..HistoryQuery::default()
        };
        for (sql, driver) in [
            (range_sql(&query, Some(LOCAL)).0, "idx_items_evictable"),
            (pinned_sql(&query, Some(LOCAL)).0, "idx_items_history"),
        ] {
            let plan = plan_of(&s, &sql);
            assert!(plan.iter().any(|step| step.contains(driver)), "{plan:?}");
            for index in ["idx_items_syncable", "idx_items_source_app"] {
                assert!(plan.iter().any(|step| step.contains(index)), "{plan:?}");
            }
            for step in plan.iter().filter(|step| {
                step.contains("clipboard_items")
                    || step.starts_with("SEARCH")
                    || step.starts_with("SCAN")
            }) {
                assert!(step.contains("COVERING INDEX"), "{step} in {plan:?}");
            }
        }
    }
}
