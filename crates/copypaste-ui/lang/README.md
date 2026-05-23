# CopyPaste UI — Localization (i18n)

Scaffolding for Slint-based translations of the CopyPaste desktop UI.

## Status

- **Catalogs present:** `en.po` (source-of-truth), `uk.po` (Ukrainian)
- **Wired into build:** not yet — the `.slint` files still contain bare string literals.
  A follow-up cleanup task will replace them with `@tr("…")` calls and add
  `slint_build::CompilerConfiguration::with_translation_domain(...)` to `build.rs`.
- **Runtime selection:** not yet — currently the OS locale will be used once wiring lands.

## File layout

```
crates/copypaste-ui/lang/
  README.md       — this file
  en.po           — English msgid catalog (canonical source)
  uk.po           — Ukrainian translation
  <locale>.po     — future translations
```

After wiring, compiled `.mo` files will live under
`target/<profile>/build/copypaste-ui-*/out/locale/<lang>/LC_MESSAGES/copypaste-ui.mo`
(produced automatically by `slint-build` from the `.po` sources).

## Slint `@tr(...)` primer

Slint has built-in gettext-style translation via the `@tr()` macro inside
`.slint` markup. Once wiring lands:

```slint
// Before
Text { text: "Settings"; }

// After
Text { text: @tr("Settings"); }
```

Plurals:

```slint
Text { text: @tr("{n} item" | "{n} items" % count); }
```

Context (for ambiguous strings):

```slint
Text { text: @tr("Menu" => "File menu label"); }
```

Every literal that appears inside `@tr("…")` must have a matching `msgid` entry
in `en.po`. Run `xtr` (or the slint LSP code action) to regenerate the template
once `.slint` files are updated.

## Adding a new locale

1. Copy `en.po` to a new file named with the target ISO code:

   ```bash
   cp crates/copypaste-ui/lang/en.po crates/copypaste-ui/lang/de.po
   ```

2. Edit the header block of the new file:
   - `Language: de\n`
   - `Language-Team: German\n`
   - `Plural-Forms:` — use the rule from
     https://www.gnu.org/software/gettext/manual/html_node/Plural-forms.html
     (e.g. for German: `nplurals=2; plural=(n != 1);`)

3. Translate each `msgstr ""` value. Leave the `msgid` lines untouched.

4. Validate the catalog locally:

   ```bash
   msgfmt --check crates/copypaste-ui/lang/de.po -o /dev/null
   ```

   On macOS install gettext via Homebrew if missing:
   `brew install gettext`.

5. Once UI wiring lands, the new locale will be picked up automatically by
   `slint-build`'s `with_translation_domain("copypaste-ui")` call —
   no `build.rs` change required per locale.

## Runtime selection (planned)

The eventual runtime API will be:

```rust
// In src/main.rs, before any window is shown:
slint::init_translations!(env!("SLINT_TRANSLATIONS_PATH"));
```

Locale resolution order will be:

1. Explicit override from CopyPaste settings (`SettingsWindow` →
   `language` field — to be added).
2. `LANG` / `LC_MESSAGES` environment variables.
3. OS-reported locale (`sys-locale` crate).
4. Fallback to `en`.

## Conventions

- Keep `msgid` strings stable. Renaming an `msgid` invalidates every
  existing translation. Prefer adding a new entry and deprecating the old one.
- Do **not** include trailing whitespace inside translatable strings —
  add it via Slint layout instead.
- Use `…` (U+2026) not `...` for ellipsis to match existing UI style.
- Wrap multi-line strings with `\n` escapes inside the `msgid` (see
  the empty-state message in `en.po`).

## Validation

```bash
# All catalogs
msgfmt --check crates/copypaste-ui/lang/en.po -o /dev/null
msgfmt --check crates/copypaste-ui/lang/uk.po -o /dev/null

# Compare msgid coverage between en and a translation
msgcmp crates/copypaste-ui/lang/uk.po crates/copypaste-ui/lang/en.po
```

## References

- Slint docs — Translations: https://docs.slint.dev/latest/docs/slint/guide/development/translations
- gettext PO format: https://www.gnu.org/software/gettext/manual/html_node/PO-Files.html
