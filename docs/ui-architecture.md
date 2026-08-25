# Frontend architecture

This document is the maintenance contract for `crates/copypaste-ui`. The UI is
one directed composition graph:

```text
design tokens → reset/globals → ui primitives → layout primitives
              → shared product components → feature model/components
              → feature patterns → feature screens → app routes/shell
```

Imports point left or stay within one feature. An app route may import a
feature's public `index.ts`; a feature may not import another feature's screen
or pattern internals. Intentionally reusable connected contracts in `capture`,
`devices`, `diagnostics`, `pairing`, `service` and `source-apps` are published
explicitly through their feature-root barrels. Settings consumes
`CaptureSetupState`, `SourceExclusions` and `DeviceNameField` from those roots,
never their screens or internal paths.

## Layers and ownership

| Layer | Owns | Must not own |
|---|---|---|
| `design/dist/css` | Generated semantic tokens and Tailwind mappings | Component selectors |
| `src/styles` | Reset, root sizing, application foreground/background, platform/theme root state | Feature geometry or descendant styling |
| `components/ui` | One DOM or interaction primitive per component | i18n, stores, IPC, feature types, screen layout |
| `components/layout` | Flow, gaps, gutters, scrolling, pane and shell sizing | Product copy, colors, data access |
| `components/shared` | Product-wide presentational compositions | Stores, IPC, feature hooks or feature data fetching |
| `features/*/model` | React-free feature types, normalization and presentation mapping | JSX or CSS classes |
| `features/*/components` | One domain concept with normalized props | Screen widths, routes, breakpoints or store queries |
| `features/*/patterns` | Feature sections, queries and controller composition | App shell or route selection |
| `features/*/screen` | Page geometry, responsive collapse and route-level states | Global navigation |
| `app/routes`, `app/shell` | Lazy screen selection, navigation and coarse shell switch | Feature internals |

Public barrels exist at `components/ui`, `components/layout`,
`components/shared`, each feature root, and intentionally reusable feature
sublayers. There is no `components/index.ts` mega-barrel. Deep cross-feature
imports into `patterns` or `screen` are prohibited.

Root `hooks/` contains only genuinely cross-feature controllers. A hook used
only by History belongs in `features/history/hooks`; feature models remain
React-free even when a screen later maps their finite ids to icons or renderers.

## Canonical foundations

Every element of one interaction type composes the same foundation.

| Foundation | Controlled modifiers | Composed by |
|---|---|---|
| `Button` | `variant=primary|secondary|ghost|danger`, `size=compact|compactIcon|sm|md|lg|icon`, `tone=neutral|danger`, `state=normal|loading` | `ActionButton`, dialog actions, navigation, all feature actions |
| `ControlSurface` | `size=compact|md|library`, `width=content|fill`, `state=normal|invalid|disabled` | `Input`, `Select`, `SearchField` |
| `ControlAdornment` | `size=compact|regular`, `tone=inherit|muted` | icons and end slots inside controls |
| `ShortcutBadge` | `size=compact|regular` | keyboard hints inside controls and menus |
| `SelectionControl` | `hitSize=compact|comfortable`; fixed 16px visual control inside the target | card-local and bulk selection |
| `Surface` | `elevation=flat|raised|overlay`, `border=none|subtle|strong`, `radius=sm|md|lg`, `tone=neutral|accent|warning|danger` | cards, `PreviewSurface`, notices and Quick Paste rows |
| `Icon` | finite name, size and weight | shared actions and feature metadata |
| `DropdownMenu` | Radix-owned menu focus, keyboard navigation and checked state | reusable menus and `MultiSelect` |
| `Tooltip` | Radix-owned trigger/content behavior; one provider in `AppProviders` | every shared icon-only `ActionButton` |

`Button` is the only raw button foundation. `ControlSurface` owns shared
input/search/select chrome. Compact controls render at 32px for a fine pointer
and preserve a 44px target for a coarse pointer; this modifier never changes
normal form inputs globally. Every icon-only button size is square by primitive
contract. `Surface` owns card and preview chrome. A new wrapper is valid only
when it adds a semantic contract; a pass-through alias is not a component.

Primitives and layouts may accept `className` as a composition escape hatch.
Feature domain components expose finite props or documented slots instead of
accepting arbitrary external geometry. State is expressed with native/ARIA or
`data-*` attributes before it is styled.

## Composition inventory

| Component or family | Primitive/layout base | Controlled contract | Main consumers |
|---|---|---|---|
| `ActionButton` | `Button` + `Icon`; delegates icon-only actions to `IconButton` | button variant/size/tone/state, icon name or glyph, control edge | Library, Devices, Settings, Capture, Diagnostics, Quick Paste |
| `IconButton` | square `Button` + `Icon` + `Tooltip` | compact/regular size, finite icon source, accessible label, control edge | all icon-only actions |
| `SearchField` | `ControlSurface` + embedded `Input` + `IconButton` + `ShortcutBadge` | value, shortcut-or-clear state, disabled | Library and Quick Paste |
| `MetadataList` | semantic `dl` grid | regular/compact density, wrapping or truncating values | inspectors and detail panes |
| `StatusCard` | `Surface` | finite status, title/detail/icon/action slots, compact density | navigation and connection summaries |
| `InspectorShell` | `PaneHeader` + `ScrollViewport` | title/header actions/body/actions/metadata slots | resizable inspectors and detail panes |
| `PreviewSurface` | `Surface` | padding and scroll plus surface modifiers | Library Inspector and detail views |
| `EmptyState`, `StateNotice`, `InlineNotice` | `Surface` + flow layouts | tone, busy/live state, finite actions | route failures, loading/empty/offline states |
| `NavigationItem` | `Button` + `Icon` + `Tooltip` | `layout=sidebar|dock`, active/disabled | desktop sidebar and mobile dock |
| `ClipCard` | `Surface` + `Button` + card-local `SelectionControl` | selection state, content kind, preview lines | measured History virtual rows |
| Clip body/media family | `Stack`, `Surface`, `Icon`, media element | normalized kind, intrinsic size/fit, masked/loading/error | ClipCard, Inspector, Quick Paste |
| Source metadata family | `Inline`, `AppIcon`, `Icon` | density, wrap, semantic badges | ClipCard and Inspector |
| History list family | `ScrollViewport` + measured ClipCards | grouping, selection, estimates replaced by measurements | LibraryScreen |
| Library toolbar/Inspector | container/flow layouts + shared controls/surfaces | search/filter/selection and item state | LibraryScreen |
| `CaptureSetup` | `Surface`, settings rows/notices, capture components | normalized snapshot or connected state | CaptureScreen and Android Settings |
| Settings tabs/index/search | `Tabs`, `PaneHeader`, `ScrollViewport`, model ids | expanded tabs versus compact ladder | SettingsScreen |
| `ApplicationShell` | `AppFrame` + navigation family | compact/expanded width and platform surface | app routes |

## Layout contracts

- `Stack` owns vertical flow; `Inline` owns horizontal flow and wrapping.
- `Grid` owns finite column/minimum-item-width options.
- `Container` owns fluid, reading and Library widths plus responsive gutters.
- `Screen` is a non-collapsing flex column with `min-inline-size: 0` and
  `min-block-size: 0`; `height=full|content` is explicit.
- `ScrollViewport` owns overflow, overscroll, focus and token padding.
- `PaneHeader` owns title/action placement, not title appearance.
- `SplitPane` owns generic panel constraints, optional collapse behavior,
  separator semantics and keyboard resizing. Its defaults are neutral; the
  Library screen supplies a 390px primary minimum, 278px inspector minimum,
  322px inspector default and 50% inspector maximum.
- `AppFrame` owns navigation/content placement. Width chooses expanded versus
  compact chrome; platform chooses capabilities, never layout.

Every flex/grid child that may shrink declares `min-inline-size: 0`; every
scrolling/filling column declares `min-block-size: 0`. A component chooses one
of intrinsic size, fill, or an explicit min/max contract. Accidental
`height: 100%`, unbounded media and overflow clipping are defects.

Internal content padding belongs to the component that draws the surface.
Outer placement and gaps belong to layouts or screens. Padding uses semantic
spacing tokens. `padding=none` is an intentional modifier for media or a parent
that already owns the inset; an omitted inset is not an implicit zero.

## Text and media overflow

Text may truncate only when the full value remains available through adjacent
context, a title, or an accessible name.

- Single-line labels use `min-inline-size: 0`, `overflow: hidden`,
  `text-overflow: ellipsis` and `white-space: nowrap` on the text owner.
- Multi-line previews use a line clamp with a matching line-height; they never
  use a fixed height that can cut half a line.
- Library text preview lines follow the normalized preference (default two).
  Code cards clamp to five complete lines.
- Inspector and detail content scroll or wrap; full content is never replaced
  by an ellipsis.
- Paths, addresses and metadata that are intentionally one line expose the full
  value with `title` or equivalent accessible context.
- Images use the shared `ClipImage` primitive, centered on a neutral preview
  surface with `object-fit: contain`. Cropping is not a consumer option.

History list payloads are previews. Inspector and expanded-reader surfaces use
the shared `useItemBody` query, which resolves the non-sensitive full value by
item id through `get_item_body`. A sensitive value never crosses that command;
its separate reveal contract remains platform-gated. If the backend cannot
return the full value, the reader shows an explicit unavailable state and does
not present the preview fragment as complete content.

Code preview source is detected by `lowlight` with a selectively registered
`highlight.js` grammar set. The resulting HAST is converted to React nodes by
`hast-util-to-jsx-runtime`; rendered clipboard content never passes through
`dangerouslySetInnerHTML`. Cards show five complete lines and the Inspector or
expanded reader shows the bounded full source. Unknown input remains escaped
plain text and is labelled `Unknown`.

## History row sizing and virtualization

`ClipCard` has no fixed block size. Its intrinsic height comes from the content
variant: configured text lines, a five-line code preview, file/link metadata,
or the image aspect contract. Card and slot padding are token-owned.

`virtualizationMetrics.ts` supplies initial scroll-space estimates for group,
fixed-tile, variable-line text, code, desktop-image and compact-image variants.
Each reservation is at least the variant's rendered cap; compact images reserve
the largest aspect-ratio box below the shared 640px shell boundary. An estimate
also receives the card's finite source/device/state metadata-unit count and
reserves every unit on its own row. An estimate never becomes an element height.
`HistoryVirtualList` owns the translated virtual canvas and passes every
mounted group and item row to TanStack Virtual's
`measureElement`, so loaded images, responsive wrapping and preference changes
replace the estimate with rendered geometry. New card variants must add a
matching estimate and remain measurable.

## Global-style exceptions

Appearance has one mode choice (`system|light|dark`) and one product-theme
choice (`midnight|aurora|ember|graphite`). The persisted keys remain `theme`
and `colorTheme`; public DOM semantics are `data-color-scheme`, `data-mode`,
`data-theme` and `data-translucency`.
`design/tokens/color/themes.*.json` owns all eight resolved palettes. Theme
previews are generated from those values, and components consume only semantic
variables. There is no independent accent preference or runtime palette writer.

Global selectors are allowed only where CopyPaste does not own the target DOM.
`AppToaster.module.css` styles Sonner's generated slots and may use narrowly
scoped `:global` selectors and `!important` where Sonner's inline sizing wins
the cascade. `OnboardingScreen.module.css` reads the application root's
normalized `data-platform` capability; it does not reach into another
component. Reduced-motion overrides in reset and modal styles are the only
other `!important` declarations. These exceptions may not become a route for
feature or descendant ownership.

`nativeAppearance.ts` synchronizes the resolved light/dark scheme with native
window chrome; it never writes design tokens. `startupFailure.ts` uses
self-contained inline fallback colors because it must remain legible when
stylesheet module evaluation is the startup failure itself.

## Responsive and platform rules

- The shell has one compact boundary at 640px. The 640–760px desktop range uses
  the narrow sidebar; narrower surfaces use the mobile dock.
- The React-free boundary value lives in `lib/layoutBreakpoints.ts`; hooks and
  width-sensitive model reservations consume that contract rather than
  duplicating a JavaScript breakpoint.
- Library shows the resizable inspector from 900px and otherwise uses the
  detail dialog. Its toolbar stays on one row and owns container-query changes
  at 760px, 600px and 520px. The count is the first optional metadata hidden;
  Search and Select collapse to icons, and expanded Search makes its siblings
  inert until focus returns to the trigger.
- Safe-area insets and coarse-pointer target sizes come from generated tokens.
- Cards, controls and metadata wrap or switch to one column at their owning
  screen/component boundary. Screen styles do not reach into leaf internals.
- macOS, Android and Windows differences come from normalized platform or
  capability data. Display strings and user-agent guesses do not select UI.
- Reduced-motion rules remove decorative motion without removing state.

## Resize and observer ownership

`ViewportMetricsProvider` owns the application's only `ResizeObserver`, using
the maintained `@juggle/resize-observer` implementation for the Chromium 53
floor. It observes the document root and multiplexes explicit container
subscriptions through one registry with stable callbacks, per-element
observe/unobserve and StrictMode-safe cleanup. `useViewportMetrics()` exposes
viewport width, height, pointer kind and size class.
`useObservedElementSize()` exposes a callback ref and measured size for the few
components whose behavior, rather than styling, depends on container geometry.

Components must not construct `ResizeObserver`, `MutationObserver` or
`IntersectionObserver`, or install persistent global resize/scroll listeners.
Purely visual responsiveness uses CSS media or container queries. The theme
store owns the single color-scheme media query; `ViewportMetricsProvider` owns
the pointer media query. A temporary capture-phase scroll listener is permitted
only while a touch selection long-press is pending and is removed with the
gesture cleanup.

## Compatibility shims

Compatibility wrappers and old-path re-exports are prohibited. Consumers move
directly to the canonical component and its finite API; obsolete wrappers are
removed with their final consumer.

## Change checklist

Before adding or changing UI, identify the geometry owner, interaction owner,
style owner and public import. Reuse the canonical foundation, add only finite
modifiers, and keep the import arrow pointed toward lower layers. Confirm every
new text surface has a deliberate wrapping/truncation rule, every surface has
token padding, every fill region has min/max constraints, and every responsive
change lives in the component, pattern or screen that owns the composition.
