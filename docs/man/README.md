# copypaste(1) man page

This directory documents how the `copypaste` man page is generated and installed.
The man page itself lives at `man/copypaste.1` (generated) and the template
scaffold at `man/copypaste.1.in`.

## Regenerating

From the repo root:

```sh
bash scripts/gen-manpage.sh
```

Behavior:

1. Builds `copypaste-cli` (`cargo build -p copypaste-cli --release`, falls back
   to debug if release fails).
2. If `help2man` is installed (`brew install help2man` on macOS,
   `apt install help2man` on Debian/Ubuntu), produces a canonical groff man
   page by introspecting `copypaste --help`.
3. Otherwise, fills `man/copypaste.1.in` using a simple `awk`/`sed` parser to
   extract the subcommand list. Output is still valid groff and viewable with
   `man -l`, but less rich than the `help2man` version.

The generator adds no Rust dependencies — pure shell + the existing build.

## Verifying locally

```sh
# Lint the shell script
bash -n scripts/gen-manpage.sh

# Render in a pager (works on the template too)
man -l man/copypaste.1.in
man -l man/copypaste.1
```

## Installing

### System-wide (Linux / macOS)

```sh
sudo install -d /usr/local/share/man/man1
sudo install -m 0644 man/copypaste.1 /usr/local/share/man/man1/copypaste.1
sudo mandb 2>/dev/null || true   # Linux only; macOS rebuilds lazily
```

Then:

```sh
man copypaste
```

### User-local (no sudo)

```sh
mkdir -p "$HOME/.local/share/man/man1"
install -m 0644 man/copypaste.1 "$HOME/.local/share/man/man1/copypaste.1"

# Ensure ~/.local/share/man is on MANPATH:
export MANPATH="$HOME/.local/share/man:${MANPATH:-}"
```

### Homebrew formula / package

Packagers should install `man/copypaste.1` to the formula's `man1` path, e.g.
in a Homebrew formula:

```ruby
man1.install "man/copypaste.1"
```

For `.deb` / `.rpm`, drop it under `/usr/share/man/man1/copypaste.1.gz`
(`gzip -9` the file before packaging).

## Notes

- Do **not** hand-edit `man/copypaste.1`; edit `man/copypaste.1.in` or update
  CLI help text in `crates/copypaste-cli/src/main.rs` and re-run the script.
- The template uses three placeholders: `@VERSION@`, `@DATE@`, `@COMMANDS@`.
  These are substituted by `scripts/gen-manpage.sh`.
- `help2man` output supersedes the template entirely when available — it
  reflects the live `--help` output for every flag and subcommand.
- The generated page is intentionally **not** checked in; CI / packagers
  regenerate it from source. Add `man/copypaste.1` to `.gitignore` if your
  workflow runs the generator in-tree.

## CI integration

A minimal release-time check:

```sh
bash scripts/gen-manpage.sh
mandoc -Tlint man/copypaste.1 | grep -E '^mandoc: .*: (ERROR|UNSUPP)' && exit 1 || true
```

Pair with the existing `scripts/completions.sh` so packages ship both man
pages and shell completions in one regeneration step.
