# `postgrest-rs` compatibility spike

Date: 2026-08-01

Repository base: `0a8f2c9fd4ec670bcc6734ce15456629074f5633`

## Decision

**Defer adoption.** Published `postgrest` 1.6.0 violates the one-TLS-stack
dependency policy by resolving `reqwest` 0.11 and `rustls` 0.21 beside the
workspace's `reqwest` 0.12 and `rustls` 0.23. Current upstream avoids that
specific duplication only by moving to `reqwest` 0.13 with TLS disabled, so its
internally-created client cannot reach the HTTPS-only Supabase endpoint.

The crate covers request spelling, but not the load-bearing validation, error
classification, response-body handling, redaction, chunking, or retry policy.
Preserving the current wire contract requires dropping through its escape hatch
to a version-specific `reqwest::RequestBuilder`. The migration therefore adds
dependency and API risk while retiring little production code.

## Versions and sources

- The workspace declares `reqwest = "0.12"`; the lockfile at this base resolves
  `0.12.28` with `default-features = false`, `json`, and `rustls-tls`.
- Published [`postgrest` 1.6.0](https://docs.rs/postgrest/1.6.0/postgrest/) is tag
  [`eb7fa6bb4f85614778b0ad183b6beced00d16ca3`](https://github.com/supabase/postgrest-rs/tree/eb7fa6bb4f85614778b0ad183b6beced00d16ca3),
  dated 2023-07-25. Its
  [manifest](https://github.com/supabase/postgrest-rs/blob/eb7fa6bb4f85614778b0ad183b6beced00d16ca3/Cargo.toml)
  requires `reqwest = "0.11"` with `rustls-tls` and no default features.
- Current upstream `master` was
  [`72cdbc157544857720cb5fae3bb04f97c6b741da`](https://github.com/supabase/postgrest-rs/commit/72cdbc157544857720cb5fae3bb04f97c6b741da)
  when inspected. Its package version is still `1.6.0`, but its unreleased
  [manifest](https://github.com/supabase/postgrest-rs/blob/72cdbc157544857720cb5fae3bb04f97c6b741da/Cargo.toml)
  requires `reqwest = "0.13"`, disables default features, and enables only
  `query`. Reqwest documents TLS as feature-gated; `query` enables no TLS
  backend ([0.13.4 features](https://docs.rs/crate/reqwest/0.13.4/features)).
- The only upstream CI job runs on Ubuntu, and neither manifest declares an
  MSRV ([current CI](https://github.com/supabase/postgrest-rs/blob/72cdbc157544857720cb5fae3bb04f97c6b741da/.github/workflows/ci.yml)).

All crate claims above come from crates.io metadata or the upstream sources at
the named commits. No fork or third-party compatibility summary was used.

## Dependency evidence

Three manifests were created outside the repository under
`/tmp/copypaste-postgrest-spike.dHEVay`: published 1.6.0 plus exact workspace
`reqwest`, current upstream at the exact revision plus workspace `reqwest`, and
the current revision with the straightforward `reqwest` 0.13 `rustls` feature
added to make its internal client HTTPS-capable. The relevant commands were:

```text
cargo metadata --format-version 1 --manifest-path <manifest> --no-deps
cargo tree --manifest-path <manifest> --duplicates
cargo tree --manifest-path <manifest> -p postgrest -e features
cargo tree --manifest-path <manifest> -i <package>@<version>
cargo tree --manifest-path <manifest> --target aarch64-apple-darwin -p postgrest
cargo tree --manifest-path <manifest> --target aarch64-linux-android -p postgrest
```

The scratch manifests pinned `reqwest = "=0.12.28"` with the workspace feature
set. Cargo resolved these relevant paths:

| Candidate | HTTP graph | TLS and crypto consequence |
|---|---|---|
| Published `postgrest = "=1.6.0"` | `postgrest 1.6.0 -> reqwest 0.11.27`, beside `reqwest 0.12.28` | Duplicates `hyper` 0.14.32/1.11.0, `hyper-rustls` 0.24.2/0.27.9, `rustls` 0.21.12/0.23.43, `tokio-rustls` 0.24.1/0.26.4, and `rustls-webpki` 0.101.7/0.103.13. Both paths use `ring` 0.17.14; no `native-tls`, OpenSSL, or AWS-LC package resolved. |
| Upstream `72cdbc1` | `postgrest -> reqwest 0.13.4`, beside `reqwest 0.12.28` | Only `query` is enabled on the 0.13 path. `cargo tree -p postgrest -e features` contains no TLS feature or TLS dependency, so the `Postgrest::new` client is unusable for Supabase HTTPS. |
| Upstream plus `reqwest` 0.13 `rustls` | Keeps both `reqwest` 0.12.28 and 0.13.4 | `rustls` 0.23.43 unifies, but both `ring` 0.17.14 and `aws-lc-sys` 0.43.0 via `aws-lc-rs` 1.17.3 resolve. This direct-dependency workaround adds a second crypto provider. |

Published 1.6.0 therefore meets the exact dependency exemption in
[`CLAUDE.md`](../../CLAUDE.md): adopting it would pull a second TLS stack into
the tree. Current upstream is not an adoptable alternative: it lacks TLS and a
git dependency is rejected by [`deny.toml`](../../deny.toml), whose source
policy allows crates.io and no git sources.

## Acceptance-test mapping

The relevant upstream APIs are implemented in the published
[`Builder`](https://github.com/supabase/postgrest-rs/blob/eb7fa6bb4f85614778b0ad183b6beced00d16ca3/src/builder.rs)
and
[`filter`](https://github.com/supabase/postgrest-rs/blob/eb7fa6bb4f85614778b0ad183b6beced00d16ca3/src/filter.rs)
modules. Their request behavior relevant here is unchanged at `72cdbc1`.

| Existing contract and tests | Crate coverage | Migration gap or risk |
|---|---|---|
| Inclusive initial bound, exact compound `or=(created_at.gt.M,and(created_at.eq.M,item_id.gt.ID))`, and ascending total order (`rest/client.rs::fetch_since_asks_for_an_inclusive_bound_and_a_total_order`, `a_known_tie_break_becomes_a_compound_keyset`; AT-23/24) | `.gte`, `.or`, `.select`, and `.order` can spell the filters. `.or` supplies the outer parentheses, so the current helper's outer pair must be removed. | `.limit(20)` emits `Range: 0-19`, not the asserted `limit=20` query. Exact compatibility needs `.build().query(&[("limit", "20")])`, returning to raw reqwest construction. Item-id validation and injection refusal remain ours. |
| Both `apikey` and per-request bearer; 30-second per-request timeout (`fetch_since_sends_both_the_apikey_and_the_user_token`) | `Postgrest::insert_header` and `Builder::auth` cover headers. | `Postgrest::new` creates and hides its own reqwest client. Applying the timeout requires `.build().timeout(...)`; the high-level `.execute()` path does not preserve the contract. The hidden client also prevents reuse of workspace `reqwest` across major versions. |
| Array upsert, `on_conflict=user_id,item_id`, merge duplicates, and `return=minimal` (`upsert_names_the_conflict_target_and_asks_for_a_merge`, body/chunk/tombstone tests; AT-16/22/52) | `.upsert(String)` and `.on_conflict("user_id,item_id")` cover POST, merge, and the conflict query. | `.upsert` hard-codes `return=representation`; exact behavior needs a post-`.build()` header override. Typed JSON becomes a caller-produced string. Whole-batch validation, explicit `deleted`, tombstone rules, signatures, empty-batch handling, and 100-row chunking remain ours. |
| Distinct 401/403/429/409/5xx/network/malformed outcomes, truncated diagnostic body, and URL/token/path redaction (`rest/error.rs`) | `.execute()` returns raw `Response` or that crate's version of `reqwest::Error`. It does not call `error_for_status` or classify PostgREST bodies. | Every existing classification and redaction test remains custom. With 1.6.0, even the transport error is `reqwest` 0.11 rather than the error type currently stored by `RestError`. |
| Network/5xx exponential retry plus single-shot 401 refresh and 429 delay on read and write (`rest/error.rs`; `sync/retry.rs`, AT-34/36/39) | No retry, refresh, `Retry-After`, or backoff API. Builder and client cloning can support a caller-owned retry closure. | The existing `backon` policy and outer recovery loop remain intact. Using the crate only changes the closure's request builder and creates no opportunity to remove a scheduler. |

A scratch program for both candidates built the exact compound keyset and
upsert requests. It passed only by using `.build()` followed by raw `.query`,
`.header`, and `.timeout` calls, confirming the escape-hatch dependency rather
than high-level coverage.

## macOS and Android

Both scratch graphs resolve for `aarch64-apple-darwin` and
`aarch64-linux-android`, and both candidates pass `cargo check` for
`aarch64-apple-darwin` on Rust 1.96.0. The published and current request-shape
scratch programs also run on arm64 macOS.

Android compilation was not established locally. Both exact workspace graphs
reach `ring` through workspace `reqwest`; `cargo check --target
aarch64-linux-android` stopped in `ring` because the Android NDK compiler
`aarch64-linux-android-clang` is not installed in this environment. This is an
environmental stop, not evidence of crate incompatibility. Upstream supplies no
Android or macOS CI evidence, so target support should be treated as unresolved
beyond Cargo target resolution and the existing workspace's own platform jobs.

Current upstream additionally fails the platform-independent runtime
requirement: without a TLS feature it cannot connect to Supabase on either
platform.

## Migration risk and revisit conditions

Migration risk is **high relative to the benefit**. It changes the reqwest error
type, client ownership, limit encoding, response preference, and retry-builder
seam while leaving all correctness and recovery logic in place. Published 1.6.0
also adds an old HTTP/TLS graph; current upstream is unreleased, source-policy
ineligible, and not HTTPS-capable as configured.

Revisit only when all of these are true:

1. A crates.io release shares the workspace reqwest major version or accepts an
   injected workspace client without another HTTP/TLS graph.
2. Its documented feature set provides HTTPS with the existing rustls crypto
   provider, without native TLS, OpenSSL, AWS-LC, or a second rustls version.
3. A prototype passes the current REST request-shape, status, redaction, and
   read/write retry tests unchanged, including query `limit`, compound keyset,
   `return=minimal`, and the per-request timeout.
4. The exact release passes macOS and Android target checks with the repository
   toolchain and Android NDK, and upstream declares an MSRV compatible with the
   workspace.

Until then, keep the existing reqwest 0.12 transport. Do not add `postgrest` to
the workspace manifests.
