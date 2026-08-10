# ADR-0022: Compare the canonical SQLite schema directly

Status: accepted

## Decision

Use `rusqlite` and SQLite's [`sqlite_schema`](https://sqlite.org/schematab.html)
table to compare every non-internal schema object with a fresh in-memory copy of
the one v2 schema. Refuse a database when any object differs; never repair or
migrate it during open.

This uses AGENTS.md dependency exemption 1: no maintained package provides this
exact canonical-object comparison. We evaluated
[`rusqlite_migration`](https://docs.rs/rusqlite_migration/latest/rusqlite_migration/struct.Migrations.html)
and [`refinery`](https://docs.rs/refinery/latest/refinery/struct.Runner.html).
Both track and apply ordered migration versions; neither proves that all tables,
indexes, triggers and virtual-table objects equal the schema this build writes.

## Consequences

The comparator remains a narrow adapter over the maintained `rusqlite` API.
Changing the canonical DDL makes existing development databases incompatible
unless a separate product decision deliberately introduces migration support.
Tests reject missing, extra and differently authored schema objects.
