# Product development

Run `python3 scripts/check-feature-ledger.py` before opening a pull request. The
machine-readable record is `docs/feature-ledger.json`; every shipped Tauri
command must belong to exactly one feature.

A product feature lands as one change containing its shared backend/Tauri
contract, macOS and Android behavior, UI and accessibility states, unit and
contract tests, and a hands-on scenario for each platform. Each scenario must
produce a screenshot, accessibility-tree or screen-reader log, and measured
latency against a stated p95 budget. Restart, offline, one relevant failure,
and release evidence are mandatory. A platform may be absent only when the
capability is removed from the product and the ledger marks it `removed`.

Use `npm --prefix crates/copypaste-ui run dev:android` for the Android emulator
and `npm --prefix crates/copypaste-ui run dev:native` on macOS. Run the scenario
named by the feature ledger and retain its evidence under `artifacts/native/`;
the nightly and release workflows upload the same evidence shape.

Performance credit is platform-specific. A credited p95 names either a JSON
measurement artifact or a scenario with an execution chain ending in a
workflow; a configured budget or prose report is not evidence. An unmeasured
platform stays `uncredited` rather than borrowing another platform's number.

Run `npm --prefix crates/copypaste-ui run test:native-parity` to exercise the
fail-closed receipt gate without native hardware. Release publication requires
same-commit macOS and physical Android receipts. Windows native-contract
evidence is available only through the `native-nightly.yml` manual dispatch;
it does not claim Windows UI or packaging coverage.

Incomplete records fail validation. Do not encode completion with TODOs,
waivers, placeholders, skipped assertions, or green jobs that lack evidence.
