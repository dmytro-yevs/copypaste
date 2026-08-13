# ADR-0023 — Coalesce history refresh with `lodash.debounce`

**Status:** accepted · 2026-08-13
**Scope:** the timer in front of the history page walk, and the one in front of
deferred-delete invalidation. Not the queries themselves.

## Decision

`crates/copypaste-ui/src/hooks/historyRefresh.ts` takes its trailing edge from
`lodash.debounce`, promoted from a transitive dependency of `usehooks-ts` to a
direct one. It supplies the two properties that matter here and that a
five-line `setTimeout` guard does not: a trailing edge that later calls
actually reset, and a `maxWait` ceiling so a stream that never pauses still
gets a walk.

`usehooks-ts` — already the repository's debounce, in `useHistoryController` —
cannot serve this boundary. `invalidateHistoryHead` is called from mutation
callbacks and from the push event listener, not from a render, so a hook is not
available to hold the timer.

**What the package cannot do, and what the module adds:** one walk is `P`
serial IPC round trips and `P` page decrypts for `P` loaded pages. A second
walk starting while the first is still reading amplifies rather than coalesces,
which no debounce knows about. The in-flight guard defers the next run and
re-enters the same debounce, so `maxWait` keeps bounding it.

## Cost

`lodash.debounce` is 3 kB minified with no dependencies of its own and is
already in the lockfile and in the bundle, so the shipped size does not change.
It has not been published since 2016. That is a completed package rather than an
abandoned one — one function, no transitive surface, and the alternative on
offer was a sixth hand-written scheduler (CLAUDE.md rule 1 lists five).

`@types/lodash.debounce` is a dev dependency; the package ships no types.

## Consequences

The bound is now statable, and stated in manifest 06 §5.1: a burst of captures
costs one walk of the loaded pages 200 ms after the last of them, and a stream
that never pauses costs at most one walk per 2000 ms. Events more than 200 ms
apart still each get a walk — that is the freshness §3.1.1 requires, not a
missing bound.
