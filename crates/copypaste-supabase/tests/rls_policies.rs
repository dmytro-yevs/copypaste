//! Beta-bonus: RLS policy SQL — pure unit tests, no live database.
//!
//! These tests parse the canonical RLS policy file
//! (`docs/supabase/rls-policies.sql`) and the table schema
//! (`docs/supabase/schema.sql`) and assert structural invariants:
//!
//!   * The file parses (well-formed text — no half-stripped placeholders).
//!   * Every per-operation policy (SELECT, INSERT, UPDATE, DELETE) carries
//!     the documented owner-scoping clause `user_id = auth.uid()`.
//!   * The table includes a `device_id` column (per the schema).
//!   * RLS is enabled AND forced on `public.clipboard_items`.
//!   * The `anon` role's privileges are explicitly revoked.
//!
//! ⚠️ Design note: the RLS pivot is `user_id`, not `device_id` — see the
//! header comment in `rls-policies.sql` for the rationale. `device_id`
//! still exists on the table (it mirrors `WireItem.origin_device_id`), but
//! it is NOT the scoping column. The historical task brief that said "check
//! device_id scoping clause" predates that design decision; these tests
//! pin the actual contract instead.

use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR for this crate is `<repo>/crates/copypaste-supabase`.
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent() // crates/
        .and_then(|p| p.parent()) // repo/
        .expect("repo root is two levels above the crate manifest")
        .to_path_buf()
}

fn read_sql(rel: &str) -> String {
    let path = repo_root().join(rel);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Lowercase + collapse all internal whitespace runs (incl. newlines) to a
/// single space. Lets us pattern-match across multi-line `create policy …
/// using ( … )` declarations without being defeated by indentation.
fn normalise(sql: &str) -> String {
    sql.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Strip line comments (everything from `--` to end-of-line) and collapse
/// whitespace. RLS comments mention `device_id` even though the actual policy
/// scopes on `user_id`; stripping comments avoids false positives.
fn strip_line_comments(sql: &str) -> String {
    sql.lines()
        .map(|line| match line.find("--") {
            Some(idx) => &line[..idx],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// File presence & parse smoke tests
// ---------------------------------------------------------------------------

#[test]
fn rls_policies_file_exists_and_is_nonempty() {
    let sql = read_sql("docs/supabase/rls-policies.sql");
    assert!(!sql.trim().is_empty(), "rls-policies.sql must not be empty");
    // Sanity: no unfilled template tokens.
    assert!(
        !sql.contains("{{") && !sql.contains("}}"),
        "rls-policies.sql contains unrendered template placeholders"
    );
}

#[test]
fn schema_file_exists_and_declares_clipboard_items() {
    let sql = read_sql("docs/supabase/schema.sql");
    let n = normalise(&sql);
    assert!(
        n.contains("create table if not exists public.clipboard_items"),
        "schema.sql must declare public.clipboard_items"
    );
}

// ---------------------------------------------------------------------------
// Per-policy scoping checks
// ---------------------------------------------------------------------------

/// Every per-operation policy must contain the canonical owner-scoping clause
/// `user_id = auth.uid()`. Whitespace is normalised so multi-line declarations
/// still match.
#[test]
fn every_policy_scopes_on_user_id_eq_auth_uid() {
    let sql = read_sql("docs/supabase/rls-policies.sql");
    let body = strip_line_comments(&sql);
    let n = normalise(&body);

    // The scoping clause appears once per policy. With FOUR operations
    // (select/insert/update/delete) and UPDATE having BOTH `using` and
    // `with check`, we expect at least 5 occurrences. Pin >=4 to keep the
    // test from being too brittle on future edits.
    let occurrences = n.matches("user_id = auth.uid()").count();
    assert!(
        occurrences >= 4,
        "expected the scoping clause `user_id = auth.uid()` to appear at \
         least once per operation (select/insert/update/delete); found {occurrences}"
    );

    // Each per-operation policy is named explicitly — assert their presence.
    for op in ["select", "insert", "update", "delete"] {
        let needle = format!("create policy clipboard_items_{op}_own");
        assert!(
            n.contains(&needle),
            "missing policy declaration `{needle}` in rls-policies.sql"
        );
    }
}

/// Each per-operation policy block must itself reference the scoping clause.
/// Slices the SQL between consecutive `create policy …` headers and asserts
/// the clause appears inside every block. This catches a regression where a
/// policy is added without scoping.
#[test]
fn each_policy_block_contains_scoping_clause() {
    let sql = read_sql("docs/supabase/rls-policies.sql");
    let body = strip_line_comments(&sql);
    let n = normalise(&body);

    // Find every "create policy" cut point.
    let cut_points: Vec<usize> = n
        .match_indices("create policy ")
        .map(|(i, _)| i)
        .collect();
    assert!(
        cut_points.len() >= 4,
        "expected at least 4 `create policy` declarations, got {}",
        cut_points.len()
    );

    // Pair each cut point with the next (or end-of-string).
    let mut bounds: Vec<(usize, usize)> = Vec::with_capacity(cut_points.len());
    for (idx, &start) in cut_points.iter().enumerate() {
        let end = cut_points.get(idx + 1).copied().unwrap_or(n.len());
        bounds.push((start, end));
    }

    for (start, end) in bounds {
        let block = &n[start..end];
        assert!(
            block.contains("user_id = auth.uid()"),
            "policy block missing scoping clause:\n---\n{block}\n---"
        );
    }
}

// ---------------------------------------------------------------------------
// RLS hardening (enable + force + anon revoke)
// ---------------------------------------------------------------------------

#[test]
fn rls_is_enabled_and_forced() {
    let sql = read_sql("docs/supabase/rls-policies.sql");
    let n = normalise(&sql);
    assert!(
        n.contains("alter table public.clipboard_items enable row level security"),
        "RLS must be enabled on public.clipboard_items"
    );
    assert!(
        n.contains("alter table public.clipboard_items force row level security"),
        "RLS must be FORCED on public.clipboard_items (so superuser bypasses are off)"
    );
}

#[test]
fn anon_role_privileges_are_revoked() {
    let sql = read_sql("docs/supabase/rls-policies.sql");
    let n = normalise(&sql);
    assert!(
        n.contains("revoke all on public.clipboard_items from anon"),
        "anon role must have all privileges revoked on clipboard_items"
    );
    assert!(
        n.contains(
            "grant select, insert, update, delete on public.clipboard_items to authenticated"
        ),
        "authenticated role must hold the four DML grants on clipboard_items"
    );
}

// ---------------------------------------------------------------------------
// Schema-level: device_id column exists (still required by WireItem)
// ---------------------------------------------------------------------------

#[test]
fn schema_declares_device_id_column() {
    let sql = read_sql("docs/supabase/schema.sql");
    let n = normalise(&sql);
    // The schema lists `device_id text not null` inside the create-table block.
    assert!(
        n.contains("device_id text not null") || n.contains("device_id text"),
        "schema.sql must declare a `device_id` column on clipboard_items \
         (mirrors WireItem.origin_device_id)"
    );
    // Also assert user_id is present and references auth.users.
    assert!(
        n.contains("user_id uuid"),
        "schema.sql must declare a `user_id` column on clipboard_items"
    );
    assert!(
        n.contains("references auth.users"),
        "user_id must FK into auth.users (drives RLS)"
    );
}

// ---------------------------------------------------------------------------
// Default user_id := auth.uid() — clients don't need to spell it out
// ---------------------------------------------------------------------------

#[test]
fn user_id_default_is_auth_uid() {
    let sql = read_sql("docs/supabase/rls-policies.sql");
    let n = normalise(&sql);
    assert!(
        n.contains("alter column user_id set default auth.uid()"),
        "user_id column must default to auth.uid() so RLS `with check` passes \
         when clients omit it"
    );
}
