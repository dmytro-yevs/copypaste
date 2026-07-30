# `components/ui` — shadcn/ui

These are [shadcn/ui](https://ui.shadcn.com) components (`new-york` style), which
by design live in the consuming repository rather than in `node_modules`. Radix
primitives underneath them provide the behaviour the accessibility contract in
`docs/rewrite/port-manifest/06-ui-behaviour.md` §4 requires: dialog focus
trapping and restoration (A11Y-4), reference-counted scroll lock (INV-19),
tablist arrow-key navigation with wrap-around (A11Y-6), `aria-expanded` /
`aria-controls` wiring (A11Y-7).

**Do not put a colour in one of these files.** They resolve their colours from
the semantic slots (`primary`, `muted`, `destructive`, `border`, `ring`, …) that
`design/tokens/semantic/tailwind.json` maps onto our tokens. That mapping is
what makes `[data-theme]` and `[data-accent]` re-tint every component at
runtime, and it is the only reason there is one palette rather than two.

To add or update a component:

```sh
cd crates/copypaste-ui && npx shadcn@latest add <component>
```

`components.json` in the crate root is already configured for it (`new-york`,
`zinc` base, `@/` alias, `lucide` icons).

> `ui.shadcn.com` is blocked by this environment's egress policy, so these files
> were written to match the canonical sources rather than fetched by the CLI.
> Re-adding a component on a networked machine will overwrite ours with
> upstream's, which is the intended direction of drift.
