# Gitleaks rule snapshot

CopyPaste vendors the default config and license from Gitleaks `v8.30.1`,
commit `83d9cd684c87d95d656c1458ef04895a7f1cbd8e`.

Exact source URLs and SHA-256 checksums live in
[`../sensitive-rules.toml`](../sensitive-rules.toml). The snapshot is MIT
licensed; its unmodified upstream license is stored beside it.

Normal builds and tests use only checked-in files. Refresh and regenerate with:

```sh
cargo run -p copypaste-sensitive-rules -- update
cargo run -p copypaste-sensitive-rules -- generate
cargo run -p copypaste-sensitive-rules -- check
```

To pin a newer release, review the selected rule IDs and overlay decisions,
then update the source metadata and checksums before running `update`.
