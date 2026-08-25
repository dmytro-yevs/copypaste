# `components/ui` — atomic primitives

The complete dependency, sizing, overflow and responsive contract lives in
[`docs/ui-architecture.md`](../../../../../docs/ui-architecture.md).

These are the application's DOM and interaction foundations. Radix and shadcn
provide the maintained behavior for dialogs, tabs, tooltips, selection and
focus management required by `docs/rewrite/port-manifest/06-ui-behaviour.md`.

This layer may import Radix, Phosphor or Lucide icons, CVA, `cn`, and token CSS.
It must not import i18n, stores, IPC, hooks, feature types, shared components,
or layouts. Feature-facing composition belongs in `components/shared`.

`Button` owns every button interaction, `ControlSurface` owns input/search/select
chrome, and `Surface` owns cards and previews. Their canonical modifiers are:

- Button: `primary | secondary | ghost | danger`,
  `compact | compactIcon | sm | md | lg | icon`, `normal | loading`.
- ControlSurface: `compact | md | library`, `content | fill`,
  `normal | invalid | disabled`.
- ControlAdornment and ShortcutBadge: `compact | regular`, with inherited or
  muted adornment tone.
- SelectionControl: `compact | comfortable` hit target with one fixed visual
  checkbox.
- Surface: elevation, border, radius and semantic tone.

Do not add raw colors or feature selectors here. CSS Modules reference semantic
tokens, while a primitive owns only its own element and state selectors.

To add or update a component:

```sh
cd crates/copypaste-ui && npx shadcn@latest add <component>
```

`components.json` in the crate root is already configured for it (`new-york`,
`zinc` base, `@/` alias, `lucide` icons).
