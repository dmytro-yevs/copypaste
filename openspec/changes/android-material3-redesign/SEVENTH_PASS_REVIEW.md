# Seventh-pass verification of `SIXTH_PASS_REVIEW`

Date: 2026-07-02  
Validation: `openspec validate android-material3-redesign --strict` — **valid**.

## Verdict

All three reopened P0 contradictions are now closed: scanner `FLAG_SECURE`, six-digit SAS, and
unapproved tablet/fold scope. The change is materially cleaner, but the claim that all SIXTH_PASS
items are closed is not yet accurate. Four residual inconsistencies remain.

## P1 residuals

### 1. Exact typography and dimensions were deferred, not specified

SIXTH_PASS #5/#6 required the spec to record exact implementation tables. `tasks.md:90–94` now asks
S1.9 to create those tables later, but the current normative spec still contains ranges:

- `android-design-system/spec.md:175`: tile `32–36`, nav pill `≈50×38`, unnamed role icon/QR/SAS sizes;
- `android-iconography/spec.md:16–23`: tile `32–36`, glyph `~16–20`;
- `tasks.md:111`: repeats `32–36` and `~16–20`.

Therefore the current spec still cannot be implemented or pixel-verified deterministically. Put the
exact `CpTypography` and `CpDimensions` tables into the normative design-system spec now. S1.9 should
implement/test those frozen values, not decide them during implementation. Update iconography and
S2.8 to reference named exact roles rather than ranges.

### 2. Full M3 mapping still has the rejected alternative path

Although D2 was corrected, the implementation contract was not:

- `tasks.md:84`: `Full ColorScheme map (or per-component overrides)`;
- `design.md:R13`: `full ColorScheme map or per-component overrides`.

This is precisely the non-equivalent “or” flagged in SIXTH_PASS #7. Choose one testable strategy.
If the intended decision is the full explicit role table stated in D2, remove the alternative from
S1.7 and R13, require every M3 `ColorScheme` role, and add a leakage test/gallery covering every M3
component actually used.

### 3. D9 contains a conditional decision followed by an unconditional golden statement

`design.md:D9` first makes tablet/fold conditional, then says goldens cover phone + tablet + fold.
The Resolved decisions section and tasks correctly make those goldens conditional, but D9 can still
be read as a committed requirement. Rewrite its final sentence to “If the S0 gate approves, goldens
also cover representative tablet and fold widths.”

## P2 editorial defect

### 4. S4 task 4.2 is duplicated

`tasks.md` contains the `4.2 Floating-pill nav` checkbox twice consecutively. Remove the first
incomplete duplicate. Duplicate task IDs corrupt issue import, progress counting, and evidence links.

## Confirmed closed

- Scanner task and resolved decision now require `FLAG_SECURE` before preview initialization.
- S8.1 now makes six-digit SAS primary and fingerprint supplemental.
- Pixel portrait is committed; tablet/fold is explicitly gated and conditional throughout tasks and
  the visual-regression spec, apart from the D9 sentence above.
- Save is removed from the async scenario and has a synchronous failure contract.
- Connected checks are assigned to concrete PR-blocking slices.
- Golden comparison defines metric fields, 0% default, and approval for nonzero tolerance.
- Resource surfaces, four notification channels, partial-span masking, settings controls, and
  feedback producers now have explicit implementation tasks.
- Decision order, audit provenance, localization allowlist, blur spike metrics, and mutation approval
  boundary are corrected.

Approval requires fixing the four rows above and rerunning strict validation plus duplicate-task-ID
and approximate-dimension searches.
