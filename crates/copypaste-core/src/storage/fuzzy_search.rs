//! Bounded spelling expansion over the existing FTS vocabulary, never payloads.

use rusqlite::{params, Connection};

use super::search::sanitize_fts5_query;

const MAX_WORD_CHARS: usize = 64;
const MAX_EXPANDED_TERMS: usize = 16;
const MAX_ALTERNATIVES: usize = 16;

pub(super) struct SearchExpression {
    pub exact: String,
    pub expanded: String,
}

impl SearchExpression {
    pub fn exact_cte(&self, exact_parameter: usize) -> String {
        if self.exact == self.expanded {
            return String::new();
        }
        format!(
            "exact_matches AS MATERIALIZED \
            (SELECT rowid FROM clipboard_fts WHERE clipboard_fts MATCH ?{exact_parameter}), "
        )
    }

    /// Prefix matches always precede spelling/infix matches. BM25 orders each
    /// tier, with a bounded transform so term frequency cannot cross tiers.
    pub fn relevance_sql(&self) -> &'static str {
        if self.exact == self.expanded {
            return "search_rank";
        }
        "CASE WHEN search_rowid IN exact_matches \
             THEN -1.0 ELSE 0.0 END + \
             search_rank / (1.0 + abs(search_rank))"
    }
}

/// Keep phrases and every original prefix; add only a bounded set of actual
/// indexed words. SQLite itself normalizes query words with unicode61, keeping
/// case, accents and token boundaries identical to the existing index.
pub(super) fn compile(conn: &Connection, raw: &str) -> rusqlite::Result<Option<SearchExpression>> {
    let Some(exact) = sanitize_fts5_query(raw) else {
        return Ok(None);
    };
    let parts = query_parts(&exact);
    let eligible: Vec<_> = parts
        .iter()
        .enumerate()
        .filter_map(|(index, part)| {
            let word = part.strip_suffix('*')?;
            let size = word.chars().count();
            (!word.contains('"') && (4..=MAX_WORD_CHARS).contains(&size)).then_some((index, word))
        })
        .take(MAX_EXPANDED_TERMS)
        .collect();
    if eligible.is_empty() {
        return Ok(Some(SearchExpression {
            expanded: exact.clone(),
            exact,
        }));
    }

    conn.execute_batch(
        "CREATE VIRTUAL TABLE IF NOT EXISTS temp.copypaste_search_vocabulary \
         USING fts5vocab(main, clipboard_fts, row); \
         CREATE VIRTUAL TABLE IF NOT EXISTS temp.copypaste_search_input USING fts5(text); \
         CREATE VIRTUAL TABLE IF NOT EXISTS temp.copypaste_search_input_terms \
         USING fts5vocab(temp, copypaste_search_input, instance); \
         DELETE FROM temp.copypaste_search_input;",
    )?;
    for (index, word) in &eligible {
        conn.execute(
            "INSERT INTO temp.copypaste_search_input(rowid, text) VALUES (?1, ?2)",
            params![*index as i64 + 1, word],
        )?;
    }
    let mut normalized = vec![None; parts.len()];
    {
        let mut stmt = conn.prepare_cached(
            "SELECT doc, min(term) FROM temp.copypaste_search_input_terms \
             GROUP BY doc HAVING count(*) = 1",
        )?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let index = row.get::<_, usize>(0)? - 1;
            let word: String = row.get(1)?;
            let chars: Vec<_> = word.chars().collect();
            if (4..=MAX_WORD_CHARS).contains(&chars.len()) {
                normalized[index] = Some(chars);
            }
        }
    }
    // No user text remains in a pooled connection after query normalization.
    conn.execute("DELETE FROM temp.copypaste_search_input", [])?;
    let mut alternatives = vec![Vec::<(u8, i64, String)>::new(); parts.len()];
    let mut stmt = conn.prepare_cached(
        "SELECT term, doc FROM temp.copypaste_search_vocabulary \
         WHERE length(term) BETWEEN 3 AND 66",
    )?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let word: String = row.get(0)?;
        let popularity: i64 = row.get(1)?;
        let chars: Vec<_> = word.chars().collect();
        for (index, _) in &eligible {
            let Some(query) = &normalized[*index] else {
                continue;
            };
            let Some(cost) = match_cost(query, &chars) else {
                continue;
            };
            let choices = &mut alternatives[*index];
            choices.push((cost, -popularity, word.clone()));
            choices.sort_unstable();
            choices.truncate(MAX_ALTERNATIVES);
        }
    }
    let expanded = parts
        .iter()
        .enumerate()
        .map(|(index, part)| {
            let choices = &alternatives[index];
            if choices.is_empty() {
                return (*part).to_owned();
            }
            let mut expression = format!("({part}");
            for (_, _, word) in choices {
                // Vocabulary words cannot inject MATCH operators or quotes.
                expression.push_str(" OR \"");
                expression.push_str(&word.replace('"', "\"\""));
                expression.push('"');
            }
            expression.push(')');
            expression
        })
        .collect::<Vec<_>>()
        .join(" AND ");
    Ok(Some(SearchExpression { exact, expanded }))
}

fn query_parts(expression: &str) -> Vec<&str> {
    let mut quoted = false;
    let mut start = 0;
    let mut parts = Vec::new();
    for (index, ch) in expression.char_indices() {
        if ch == '"' {
            quoted = !quoted;
        } else if !quoted && expression[index..].starts_with(" AND ") {
            parts.push(&expression[start..index]);
            start = index + 5;
        }
    }
    parts.push(&expression[start..]);
    parts
}

fn match_cost(query: &[char], word: &[char]) -> Option<u8> {
    if word.starts_with(query) {
        return None; // Already covered by the original prefix expression.
    }
    if word.windows(query.len()).any(|part| part == query) {
        return Some(1);
    }
    // Avoid fuzzy numeric identifiers, hashes and very short words.
    if !query.iter().all(|ch| ch.is_alphabetic()) {
        return None;
    }
    let edits = if query.len() >= 8 { 2 } else { 1 };
    prefix_distance(query, word, edits)
}

/// Banded optimal-string-alignment distance; adjacent swaps cost one edit.
/// Taking the best endpoint also supports a mistyped, unfinished word.
fn prefix_distance(query: &[char], word: &[char], edits: u8) -> Option<u8> {
    let max = query.len() + usize::from(edits);
    let word = &word[..word.len().min(max)];
    if word.len() + usize::from(edits) < query.len() {
        return None;
    }
    let infinity = edits + 1;
    let mut previous_previous = [infinity; MAX_WORD_CHARS + 3];
    let mut previous = [infinity; MAX_WORD_CHARS + 3];
    for (index, cell) in previous.iter_mut().enumerate().take(usize::from(edits) + 1) {
        *cell = index as u8;
    }
    for (i, &left) in query.iter().enumerate() {
        let i = i + 1;
        let mut current = [infinity; MAX_WORD_CHARS + 3];
        current[0] = (i as u8).min(infinity);
        let from = i.saturating_sub(usize::from(edits)).max(1);
        let to = (i + usize::from(edits)).min(word.len());
        for j in from..=to {
            current[j] = (previous[j] + 1)
                .min(current[j - 1] + 1)
                .min(previous[j - 1] + u8::from(left != word[j - 1]));
            if i > 1 && j > 1 && left == word[j - 2] && query[i - 2] == word[j - 1] {
                current[j] = current[j].min(previous_previous[j - 2] + 1);
            }
        }
        if current[from..=to].iter().all(|distance| *distance > edits) {
            return None;
        }
        previous_previous = previous;
        previous = current;
    }
    previous[query.len().saturating_sub(usize::from(edits))..=word.len()]
        .iter()
        .copied()
        .min()
        .filter(|distance| *distance <= edits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_conjunctions_only_outside_phrases() {
        assert_eq!(
            query_parts("\"first AND second\" AND third*"),
            ["\"first AND second\"", "third*"]
        );
    }

    #[test]
    fn bounded_distance_handles_edits_and_unicode_prefixes() {
        for (query, word, expected) in [
            ("meetin", "meeting", Some(0)),
            ("meetin", "meting", Some(1)),
            ("meetin", "meetingroom", Some(0)),
            ("meetin", "greeting", None),
            ("meetin", "xmeeting", Some(1)),
            ("meetign", "meeting", Some(1)),
            ("привті", "привіт", Some(1)),
        ] {
            assert_eq!(
                prefix_distance(
                    &query.chars().collect::<Vec<_>>(),
                    &word.chars().collect::<Vec<_>>(),
                    1
                ),
                expected,
                "{query}: {word}"
            );
        }
    }

    #[test]
    fn expands_two_errors_in_long_words_and_bounds_alternatives() {
        use super::super::test_support::{item, store, T0};
        let s = store();
        let hit = s.insert(item("clipboard", T0)).unwrap();
        assert_eq!(s.search("clpiboad", 10).unwrap()[0].id, hit.id);
        assert!(s.search("clpxbozd", 10).unwrap().is_empty());
        for index in 0..40 {
            s.insert(item(&format!("preclipboard{index}"), T0 + index + 1))
                .unwrap();
        }
        let conn = s.conn().unwrap();
        let expression = compile(&conn, "clipboard").unwrap().unwrap();
        assert_eq!(
            expression.expanded.matches(" OR ").count(),
            MAX_ALTERNATIVES
        );
        assert!(expression.expanded.starts_with("(clipboard* OR "));
        drop(conn);
        assert_eq!(s.search("clipboard", 1).unwrap()[0].id, hit.id);
    }
}
