# Mobile navigation

The compact CopyPaste shell uses the main-tab geometry and interactions from
Telegram for Android, inspected on October 8, 2026.

## Reference implementation

- [DialogsActivity](https://github.com/DrKLO/Telegram/blob/master/TMessagesProj/src/main/java/org/telegram/ui/DialogsActivity.java): 56 dp capsule, 8 dp outer margins.
- [MainTabsActivity](https://github.com/DrKLO/Telegram/blob/master/TMessagesProj/src/main/java/org/telegram/ui/MainTabsActivity.java): 328 dp capsule width cap, repeat activation scrolls the current screen up, page swipes, root Back returns to the first tab.
- [MainTabsLayout](https://github.com/DrKLO/Telegram/blob/master/TMessagesProj/src/main/java/org/telegram/ui/MainTabsLayout.java): text-dependent tab widths; horizontal padding passes of 16, 8, and 4 dp; text sizes of 12, 12, and 10 dp; hold-and-drag selection; spring press response.
- [GlassTabView](https://github.com/DrKLO/Telegram/blob/master/TMessagesProj/src/main/java/org/telegram/ui/Components/glass/GlassTabView.java): 24 dp icon artwork, bold 12 dp labels, extra-bold selected labels, 320 ms decelerating selection, 9% selected tint.
- [BlurredBackgroundProviderImpl](https://github.com/DrKLO/Telegram/blob/master/TMessagesProj/src/main/java/org/telegram/ui/Components/blur3/drawable/color/impl/BlurredBackgroundProviderImpl.java): translucent glass surface, 0.4 dp stroke, subtle shadow.

## CopyPaste contract

Use the shared shadcn `NavigationBar`, `NavigationItem`, `OutlinedContainer`,
and `AppTheme` on Android, macOS, and Windows. The bar has its own normalized
component scale. Its icon size and logical dimensions remain constant when
shadcn applies platform scaling to the rest of the application.

The standard capsule is 56 logical pixels high with 48-pixel targets, 24-pixel
Lucide icons, and 12-pixel labels. Its maximum width is 328 pixels, with 8-pixel
outer margins and a 4-pixel internal inset. Tab widths follow their label
measurements and the reference padding passes. Narrow layouts can use 10-pixel
labels. User accessibility text enlargement retains 12-pixel base typography
and increases the bar height instead of reducing the user's chosen text scale.

The surface uses shadcn's clipped backdrop blur. The content remains visible and
interactive outside the capsule. Main lists include the footer clearance so
their final items can be scrolled above it. The bar hides for the keyboard and
bottom overlays.

Tapping a tab activates its retained screen and animates its selection without
highlighting intermediate tabs. Swiping the compact content pages
updates the tab selection continuously. Repeating a tab activation pops its
detail route and scrolls its main list to the top. Holding a tab for 375 ms
allows previewing selection by dragging across the bar; release commits the
nearest tab and cancellation restores the original selection. Back pops a
detail first, then returns to History from another root tab.
Tab clicks wait until the current page transition or content drag ends, matching
the reference guard. Programmatic navigation can supersede a pending transition.

The application retains its own destination names, Lucide icons, theme colors,
and business actions. A selection pulse animates the app's Lucide icons.
Gaussian backdrop blur and shared Flutter motion provide
the glass and gesture effects across all three platforms. Reduced motion skips
the page, selector, and press animations.

## Verification

Widget tests assert rendered geometry and typography on all three platform
variants, swipe and scroll retention, reactivation, hold/drag/cancel, Back,
reduced motion, keyboard visibility, safe areas, and 200% text scaling.
