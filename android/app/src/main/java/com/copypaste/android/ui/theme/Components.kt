@file:OptIn(ExperimentalMaterial3Api::class)

package com.copypaste.android.ui.theme

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.scaleIn
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.ArrowBack
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ProvideTextStyle
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.copypaste.android.Settings


// ---------------------------------------------------------------------------
// Shared design-system components — single source of truth for chrome that
// must look identical on every screen. v0.5.3 retune: deeper surface colors,
// accent #3592ff, hairline borders, shadow-equivalent elevation.
//
//   • Compact IDE-style header on the #1e2024 panel surface (NOT the blue
//     accent header Material defaults to). This is what makes the History,
//     Settings, Pair, Onboarding and Permissions screens read as siblings.
//     The status-bar inset is applied via windowInsets (not a fixed height)
//     so the header is never clipped under a notch or display cutout.
//   • Rounded 12 dp cards on the elevated surface, single 1 dp hairline border.
//   • Grey uppercase section labels (Apple grouped headers — NOT accent blue).
//
// Spacing scale: 4 / 8 / 12 / 16 / 24 dp. Keep new padding on this grid.
// ---------------------------------------------------------------------------

/**
 * Standard compact header — liquid-glass, floating (CopyPaste-6krb parity fix).
 *
 * When [translucent] is true (default: reads from the "copypaste" SharedPreferences
 * key "translucency"), the container is the §2 glass fill at GLASS_ALPHA so the
 * opaque window canvas bleeds through for a frosted/glass look. When false, the
 * bar is the fully opaque theme panel surface — the pre-glass solid look. All
 * text/icon colors come from the active light/dark ramp (LocalCpColors).
 *
 * **CopyPaste-6krb**: The header is now FLOATING — [RoundedCornerShape(14.dp)] with
 * 8 dp horizontal inset padding, matching the styleguide `.app-header` treatment and
 * the web ViewShell header. This mirrors the FloatingTabBar pattern (same inset+radius
 * approach). When translucent, a soft float shadow appears via [glassFloatShadow].
 *
 * windowInsets defaults to [TopAppBarDefaults.windowInsets] so the bar
 * automatically pads its content below the status-bar / display-cutout on
 * edge-to-edge screens. Do NOT pass a fixed height — that would clip the
 * header on notched phones by capping the total height before the inset is
 * accounted for.
 */
@Composable
fun CopyPasteTopBar(
    title: String,
    showBackButton: Boolean = false,
    onBack: () -> Unit = {},
    backContentDescription: String = "Back",
    actions: @Composable (androidx.compose.foundation.layout.RowScope.() -> Unit) = {},
    windowInsets: WindowInsets = TopAppBarDefaults.windowInsets,
    // §3 translucency: reads the pref by default; callers may override.
    translucent: Boolean = rememberTranslucency(),
) {
    // Active light/dark ramp — read once so the bar themes in lockstep (§1).
    val c = LocalCpColors.current
    val dark = isDarkTheme()

    // Fixed radius (STYLEGUIDE §5 --r-card 13dp) — no skin.
    val headerRadius = 13.dp
    val headerShape = RoundedCornerShape(headerRadius)

    // Float shadow only when the surface is translucent (glass float).
    val showHeaderShadow = translucent

    // §2/P0: outer Box carries the horizontal inset padding + float shadow so the header
    // appears to hover above content — the liquid-glass floating feel.
    // The TranslucentSurface fills and rounds the clipped area; TopAppBar is transparent.
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 8.dp)
            .then(if (showHeaderShadow) Modifier.glassFloatShadow(GlassTier.GLASS, radius = headerRadius) else Modifier),
    ) {
        TranslucentSurface(
            shape = headerShape,
            translucent = translucent,
            dark = dark,
            solid = MaterialTheme.colorScheme.surface,
            modifier = Modifier.matchParentSize(),
            // Top bars are the styleguide tier-1 .surface-glass recipe.
            // The glass rim gives edge definition on the rounded floating header.
            tier = GlassTier.GLASS,
            hairline = translucent, // hairline on floating glass; none on solid
            content = {},
        )
        TopAppBar(
            title = {
                // m3xc: view title at styleguide Heading/18/600 (headlineSmall),
                // not the compact 14sp titleLarge sub-header tier.
                Text(
                    text = title,
                    style = MaterialTheme.typography.headlineSmall,
                    color = c.text,
                )
            },
            navigationIcon = {
                if (showBackButton) {
                    IconButton(onClick = onBack) {
                        Icon(
                            // 9730: outlined family for back glyph (HistoryActivity already
                            // uses Outlined; styleguide is outline-first for nav icons).
                            Icons.AutoMirrored.Outlined.ArrowBack,
                            contentDescription = backContentDescription,
                            tint = c.dim,
                            modifier = Modifier.size(18.dp),
                        )
                    }
                }
            },
            actions = actions,
            colors = TopAppBarDefaults.topAppBarColors(
                containerColor             = Color.Transparent, // glass backdrop carries the fill
                titleContentColor          = c.text,
                actionIconContentColor     = c.dim,
                navigationIconContentColor = c.dim,
            ),
            // Apply the status-bar / display-cutout inset as TOP PADDING so the
            // bar's content sits *below* the notch, never under it. A hard fixed
            // height must NOT be set here — it would clip the header on notched
            // phones because the inset eats into the fixed total height.
            windowInsets = windowInsets,
        )
    }
}

/**
 * Rounded elevated card on the Darcula grey ramp with a hairline outline.
 *
 * [accent] tints the border (e.g. danger for a missing required permission,
 * success for a granted one) without flooding the whole card with color — this
 * is closer to the restrained macOS look than Material's filled containers.
 *
 * This is the CANONICAL glass surface (PARITY-SPEC §2): when [translucent]
 * (default: reads from SharedPreferences), the container is the §2 warm-near-
 * white (light) / deep (dark) glass fill so the opaque canvas behind it bleeds
 * through. When false, the card is the fully opaque theme elevated surface.
 *
 * Styleguide tier-2 .surface-card: 12 dp radius (parity with macOS 12 px, PG-57),
 * bright .5px white glass rim, soft tinted float shadow (0 4px 14px rgb(60 60 90 /.14)).
 * [accent] still tints
 * a SEMANTIC border (danger/success) when the caller overrides the default — that
 * sits over the glass rim so per-screen status cards keep their colour cue.
 */
@Composable
fun CopyPasteCard(
    modifier: Modifier = Modifier,
    accent: Color = MaterialTheme.colorScheme.outline,
    // §3 translucency: reads the pref by default; callers may override.
    translucent: Boolean = rememberTranslucency(),
    content: @Composable (androidx.compose.foundation.layout.ColumnScope.() -> Unit),
) {
    val dark = isDarkTheme()

    // Fixed card radius (STYLEGUIDE §5 --r-card 13dp) — no skin.
    val cardRadius = 13.dp
    val cardShape = RoundedCornerShape(cardRadius)

    // Only paint an explicit Material border when the caller overrides `accent`
    // with a SEMANTIC tint; the default outline is superseded by the bright glass
    // rim that TranslucentSurface draws.
    val semanticBorder = accent != MaterialTheme.colorScheme.outline

    // Card float shadow only when translucent.
    val showCardShadow = translucent

    // vk12: drop Material tonal elevation entirely; the soft tinted float shadow
    // is drawn behind the card via glassFloatShadow (CARD tier 0 4px 14px).
    // Shadow radius tracks cardRadius so the shadow silhouette matches the corner clip.
    Card(
        modifier = modifier
            .fillMaxWidth()
            .then(if (showCardShadow) Modifier.glassFloatShadow(GlassTier.CARD, cardRadius) else Modifier),
        shape = cardShape,
        colors = CardDefaults.cardColors(
            containerColor = Color.Transparent,
            contentColor   = MaterialTheme.colorScheme.onSurface,
        ),
        // Semantic tint border only; glass rim otherwise (1k3i). Opaque solid look
        // (translucency off) keeps a 1dp hairline so the card edge stays visible.
        border = when {
            semanticBorder -> BorderStroke(1.dp, accent)
            !translucent   -> BorderStroke(1.dp, MaterialTheme.colorScheme.outline)
            else           -> null
        },
        // No Material tonal elevation (the float shadow replaces it).
        elevation = CardDefaults.cardElevation(
            defaultElevation   = 0.dp,
            pressedElevation   = 0.dp,
            focusedElevation   = 0.dp,
            hoveredElevation   = 0.dp,
            draggedElevation   = 0.dp,
            disabledElevation  = 0.dp,
        ),
    ) {
        TranslucentSurface(
            shape = cardShape,
            translucent = translucent,
            dark = dark,
            solid = MaterialTheme.colorScheme.surfaceContainerHigh,
            tier = GlassTier.CARD,
            contentColor = MaterialTheme.colorScheme.onSurface,
        ) {
            Column(content = content)
        }
    }
}

/**
 * Theme-correct glass fill for a dialog/modal surface (PARITY-SPEC §8).
 *
 * Dialogs are a hair more opaque than cards so they read as a distinct layer
 * over the dimmed scrim: we use the §2 glass fill but floor the alpha so text
 * stays legible against whatever is behind. When translucency is off, returns
 * the opaque elevated surface. Call from a @Composable site.
 */
@Composable
fun glassDialogContainerColor(translucent: Boolean = rememberTranslucency()): Color {
    val dark = isDarkTheme()
    val solid = MaterialTheme.colorScheme.surfaceContainerHigh
    if (!translucent) return solid
    // Styleguide .surface-strong: a flat 0.92 fill (zd35/mjwc — was 0.86) so the
    // modal reads as a distinct, near-opaque layer over the dim scrim and the
    // dialog text never washes out.
    val fill = if (dark) GlassFillDark else GlassFillLight
    return fill.copy(alpha = GlassTier.STRONG.lightAlphaTop)
}

/**
 * Glass restyle of Material [AlertDialog] (PARITY-SPEC §8, audit #6/#10, P0 blur).
 *
 * Appearance only — the LOGIC (callbacks, button content, dismiss) is whatever
 * the caller passes. Built on a bare [Dialog] + [TranslucentSurface] so the
 * modal gets a REAL API-31 RenderEffect frosted backdrop (flat §8 tint fallback
 * < 31), the §4 modal radius (16 dp), a §4 hairline border, and Material's
 * dimmed scrim behind it. The slot layout mirrors Material's AlertDialog (title,
 * supporting text, then a trailing buttons row: dismiss left of confirm) so the
 * call-site signature is a near drop-in. Title/text colors come from the active
 * ramp; the caller styles its own buttons (destructive actions in `c.err`).
 */
@Composable
fun GlassAlertDialog(
    onDismissRequest: () -> Unit,
    confirmButton: @Composable () -> Unit,
    modifier: Modifier = Modifier,
    dismissButton: (@Composable () -> Unit)? = null,
    title: (@Composable () -> Unit)? = null,
    text: (@Composable () -> Unit)? = null,
    translucent: Boolean = rememberTranslucency(),
    properties: DialogProperties = DialogProperties(),
) {
    val c = LocalCpColors.current
    val dark = isDarkTheme()

    // Fixed modal radius (STYLEGUIDE §5 --r-card 13dp) — no skin.
    val modalRadius = 13.dp
    val dialogShape = RoundedCornerShape(modalRadius)

    // Float shadow only when translucent.
    val showDialogShadow = translucent

    Dialog(
        onDismissRequest = onDismissRequest,
        properties = properties,
    ) {
        // Transparent Surface; TranslucentSurface supplies the .surface-strong
        // frosted blur, the .92 fill and the bright glass rim. vk12: the soft
        // tinted modal float shadow (0 20px 60px) replaces Material elevation.
        Surface(
            modifier = modifier
                .widthIn(min = 280.dp, max = 560.dp)
                .then(if (showDialogShadow) Modifier.glassFloatShadow(GlassTier.STRONG, modalRadius) else Modifier),
            shape = dialogShape,
            color = Color.Transparent,
            border = if (translucent) null else BorderStroke(1.dp, c.border),
            shadowElevation = if (translucent) 0.dp else 6.dp,
        ) {
            TranslucentSurface(
                shape = dialogShape,
                translucent = translucent,
                dark = dark,
                tier = GlassTier.STRONG,
                // Dialogs use the higher-floor (0.92) strong fill so text stays
                // legible over the scrim. Passing it as `solid` makes the no-blur
                // (< 31 / translucency-off) path match the styleguide exactly.
                solid = glassDialogContainerColor(translucent),
                contentColor = c.text,
            ) {
                Column(modifier = Modifier.padding(24.dp)) {
                    if (title != null) {
                        CompositionLocalProvider(LocalContentColor provides c.text) {
                            ProvideTextStyle(
                                MaterialTheme.typography.titleLarge.copy(color = c.text),
                            ) { title() }
                        }
                        Spacer(Modifier.size(16.dp))
                    }
                    if (text != null) {
                        CompositionLocalProvider(LocalContentColor provides c.dim) {
                            ProvideTextStyle(
                                MaterialTheme.typography.bodyMedium.copy(color = c.dim),
                            ) { text() }
                        }
                        Spacer(Modifier.size(24.dp))
                    }
                    // Trailing buttons row: dismiss left of confirm (Material order).
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        if (dismissButton != null) dismissButton()
                        confirmButton()
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// IdeSwitch — bespoke "Liquid Glass" toggle (PARITY-SPEC §7, audit P1 #1).
//
// One geometry across both platforms: a 34×18 dp track with a 12 dp WHITE thumb
// in BOTH states (the Material default unchecked thumb was a smaller `c.dim`
// dot). Accent track when checked, `c.elevated` + `c.border` hairline when
// unchecked. NO glow/state-layer halo. The thumb glides with tween(120) — the
// §11 "instant" feel — and the track color cross-fades over the same window.
//
// Drawn by hand (Box + offset/animateDpAsState) rather than Material Switch so
// the exact 34×18 / 12 dp geometry and the no-glow requirement are guaranteed;
// Material's Switch enforces its own touch target with a pressed state-layer we
// cannot fully suppress. `toggleable` (no indication) supplies the click +
// a11y Switch role without any ripple/glow.
// ---------------------------------------------------------------------------

/**
 * Custom 34×18 dp switch with a 12 dp white thumb in both states (§7).
 *
 * @param checked  current on/off state.
 * @param onCheckedChange invoked with the toggled value when tapped (null = read-only).
 * @param enabled  when false, the control is dimmed to §4 disabled opacity (0.40)
 *                 and taps are ignored.
 */
@Composable
fun IdeSwitch(
    checked: Boolean,
    onCheckedChange: ((Boolean) -> Unit)?,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    // CopyPaste-aod: accessibility label so TalkBack announces "<name>, on/off"
    // instead of a bare "Switch, on". Optional so existing call sites that merge the
    // switch into a labelled parent row (mergeDescendants) can leave it null.
    name: String? = null,
) {
    val c = LocalCpColors.current

    // §7 geometry. Thumb travels from the left inset to (track − thumb − inset).
    val trackW = 34.dp
    val trackH = 18.dp
    val thumb = 12.dp
    val inset = 3.dp

    val disabledAlpha = if (enabled) 1f else 0.40f

    // §11 instant (120ms) thumb glide + track cross-fade — no glow.
    val thumbOffset by animateDpAsState(
        targetValue = if (checked) trackW - thumb - inset else inset,
        animationSpec = tween(120, easing = EaseStandard),
        label = "ideSwitchThumb",
    )
    // 1vgu: styleguide closed track = rgb(--ide-mute / .5) grey (was c.elevated).
    val trackColor by animateColorAsState(
        targetValue = if (checked) accentFill() else c.mute.copy(alpha = 0.5f),
        animationSpec = tween(120, easing = EaseStandard),
        label = "ideSwitchTrack",
    )

    // toggleable with indication=null → click + Switch a11y role, NO ripple/glow.
    val clickMod = if (enabled && onCheckedChange != null) {
        Modifier.toggleable(
            value = checked,
            enabled = true,
            role = Role.Switch,
            interactionSource = remember { MutableInteractionSource() },
            indication = null,
            onValueChange = onCheckedChange,
        )
    } else {
        Modifier
    }

    // CopyPaste-aod: announce a human on/off state, and a name when supplied, so the
    // switch is never read as a bare "Switch, on/off" with no context.
    val a11yMod = Modifier.semantics {
        stateDescription = if (checked) "On" else "Off"
        if (name != null) contentDescription = name
    }

    Box(
        modifier = modifier
            .then(clickMod)
            .then(a11yMod)
            .size(width = trackW, height = trackH)
            .alpha(disabledAlpha)
            .clip(RoundedCornerShape(percent = 50))
            // 1vgu: styleguide switch has NO border in either state — the mute@.5
            // closed track and accent open track read on their own.
            .drawBehind {
                drawRoundRect(
                    color = trackColor,
                    cornerRadius = CornerRadius(size.height / 2f),
                )
            },
        contentAlignment = Alignment.CenterStart,
    ) {
        Box(
            modifier = Modifier
                .offset(x = thumbOffset)
                .size(thumb)
                .clip(CircleShape)
                // §7: white thumb in BOTH states (no glow shadow).
                .drawBehind { drawCircle(Color.White) },
        )
    }
}

/**
 * Apple grouped section header (PARITY-SPEC §3): uppercase, 11 sp semibold,
 * tertiary GREY (`c.faint`) — NOT accent blue — with wide tracking. Apple
 * section headers are grey, not blue. 8 dp grid padding.
 */
@Composable
fun SectionLabel(
    text: String,
    modifier: Modifier = Modifier,
) {
    val c = LocalCpColors.current
    Text(
        // §3: uppercase Apple section header.
        text = text.uppercase(),
        style = MaterialTheme.typography.titleMedium.copy(
            fontSize      = 11.sp,
            fontWeight    = FontWeight.SemiBold,
            letterSpacing = 0.6.sp,   // tracking-wide
        ),
        // CopyPaste-bdac.89: canonical section label uses --ide-dim (same as macOS SectionHeader.tsx).
        // 5jkb had changed to faint but parity audit shows macOS uses dim (higher contrast). Aligned.
        color = c.dim,
        // CopyPaste-aod: mark as a heading so TalkBack users can jump between sections.
        modifier = modifier
            .semantics { heading() }
            .padding(start = 16.dp, top = 16.dp, bottom = 4.dp),
    )
}



// ---------------------------------------------------------------------------
// CopyPasteButton — unified styleguide button (k9ht).
//
// One component for the styleguide's button variants, all coloured from
// LocalCpColors and using the --radius-ctl 9dp control radius:
//
//   PRIMARY      accent fill + white label; press → accentPress (#0070EB light).
//   SECONDARY    glass: translucent white@.5 + .5px white hairline (tier-1 glass);
//                text colour = theme text. Falls back to a flat tint < API 31.
//   DANGER       danger@.15 tint fill + danger label (the soft destructive tier).
//   DANGER_SOLID danger fill + white label (the loud destructive tier).
//   GHOST        transparent + faint label (low-emphasis text action).
//
// Icon-only buttons use [CopyPasteIconButton] (28dp glyph inside a 44dp invisible
// hit target). Per-screen agents adopt these at their call sites; this commit
// only introduces the shared component (no global call-site rewrite).
// ---------------------------------------------------------------------------

enum class ButtonVariant { PRIMARY, SECONDARY, DANGER, DANGER_SOLID, GHOST }

/**
 * Shared styleguide button. [variant] selects the fill/label recipe; everything
 * is coloured from [LocalCpColors] so it themes light/dark in lockstep. Radius
 * is the --radius-ctl 9dp control token. Press feedback is a colour shift (no
 * Material state-layer halo). [enabled] dims to 0.40 and blocks taps.
 */
@Composable
fun CopyPasteButton(
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    variant: ButtonVariant = ButtonVariant.PRIMARY,
    enabled: Boolean = true,
    translucent: Boolean = rememberTranslucency(),
    content: @Composable RowScope.() -> Unit,
) {
    val c = LocalCpColors.current
    val dark = isDarkTheme()

    // Fixed control radius (STYLEGUIDE §5 --r-ctl 8dp) — no skin.
    val shape = RoundedCornerShape(8.dp)

    val interaction = remember { MutableInteractionSource() }
    val pressed by interaction.collectIsPressedAsState()

    // Per-variant fill (background) + label colour. Secondary is glass, so its
    // background is handled separately via TranslucentSurface below.
    val labelColor = when (variant) {
        ButtonVariant.PRIMARY      -> onAccent()
        ButtonVariant.SECONDARY    -> c.text
        ButtonVariant.DANGER       -> c.err
        ButtonVariant.DANGER_SOLID -> Color.White
        ButtonVariant.GHOST        -> c.faint
    }
    val fill = when (variant) {
        // Primary press → styleguide --ide-accent-press; resting → accent.
        ButtonVariant.PRIMARY      -> if (pressed) accentFill() else accentFill()
        ButtonVariant.DANGER       -> c.err.copy(alpha = if (pressed) 0.22f else 0.15f)
        ButtonVariant.DANGER_SOLID -> if (pressed) c.err.copy(alpha = 0.88f) else c.err
        ButtonVariant.GHOST        -> if (pressed) hoverOverlay() else Color.Transparent
        ButtonVariant.SECONDARY    -> Color.Transparent // glass draws its own fill
    }
    val disabledAlpha = if (enabled) 1f else 0.40f

    val core: @Composable () -> Unit = {
        Row(
            modifier = Modifier
                .heightIn(min = 36.dp)
                .padding(horizontal = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            CompositionLocalProvider(LocalContentColor provides labelColor) {
                ProvideTextStyle(
                    MaterialTheme.typography.labelLarge.copy(
                        color = labelColor,
                        fontWeight = FontWeight.SemiBold,
                    ),
                ) { content() }
            }
        }
    }

    val clickMod = modifier
        .clip(shape)
        .alpha(disabledAlpha)
        .clickable(
            enabled = enabled,
            role = Role.Button,
            interactionSource = interaction,
            indication = null,
            onClick = onClick,
        )

    if (variant == ButtonVariant.SECONDARY) {
        // Glass secondary — tier-1 .surface-glass recipe (translucent white@.5 +
        // .5px white hairline + blur). Falls back to a flat tint < API 31.
        Box(modifier = clickMod) {
            TranslucentSurface(
                shape = shape,
                translucent = translucent,
                dark = dark,
                solid = c.elevated,
                tier = GlassTier.GLASS,
                contentColor = labelColor,
            ) { core() }
        }
    } else {
        Box(
            modifier = clickMod.drawBehind { drawRect(fill) },
            contentAlignment = Alignment.Center,
        ) { core() }
    }
}

/**
 * Icon-only button (k9ht icon variant). A [glyphSize] icon centred inside a
 * [hitTarget] invisible touch area (styleguide: 28px glyph, 44px hit target).
 * Tint defaults to the theme dim; press has no halo (clickable indication=null).
 */
// ---------------------------------------------------------------------------
// Shared settings row composables — extracted from SettingsActivity so they
// can be reused by other screens (CopyPaste-bdac.11).
//
// SettingsRow       — label/subtitle + trailing IdeSwitch (toggle row)
// SettingsNavRow    — label/subtitle + optional leading icon (navigation row,
//                     no trailing control; tapping navigates to another screen)
//
// Both use the fixed §5 comfortable spacing — density modes were removed
// (CopyPaste-xruv, §2/§12).
// ---------------------------------------------------------------------------

/**
 * A label/subtitle + trailing [IdeSwitch] settings toggle row (CopyPaste-bdac.11).
 *
 * Extracted from SettingsActivity's private `SettingsRow` so other screens can
 * use the same primitive without copy-pasting. Density-aware: compact → 8dp
 * vertical pad + bodyMedium title; spacious → 16dp; comfortable → 12dp.
 *
 * Accessibility: the row merges descendants so TalkBack reads
 * "<title>, <subtitle>, On/Off" as a single node instead of separate stops.
 *
 * @param title         Main label text.
 * @param subtitle      Secondary description text.
 * @param checked       Current toggle state.
 * @param onCheckedChange Called with the new value when toggled.
 */
@Composable
fun SharedSettingsRow(
    title: String,
    subtitle: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
    modifier: Modifier = Modifier,
) {
    val c = LocalCpColors.current
    // §5 fixed comfortable spacing — density modes removed (CopyPaste-xruv, §2/§12).
    val vertPad = 12.dp
    Row(
        // CopyPaste-aod: merge the title + subtitle + switch into ONE TalkBack node
        // labelled with the title so it reads "<title>, <subtitle>, On/Off" instead
        // of the title/subtitle and a context-free "Switch, on" as separate stops.
        modifier = modifier
            .fillMaxWidth()
            .semantics(mergeDescendants = true) {}
            .padding(horizontal = 16.dp, vertical = vertPad),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Column(modifier = Modifier
            .weight(1f)
            .padding(end = 12.dp)) {
            Text(
                text = title,
                style = MaterialTheme.typography.bodyLarge,
                color = c.text,
            )
            Text(
                text = subtitle,
                style = MaterialTheme.typography.bodySmall,
                color = c.dim,
            )
        }
        IdeSwitch(
            checked = checked,
            onCheckedChange = onCheckedChange,
            name = title,
        )
    }
}

/**
 * A label/subtitle navigation row with optional leading icon (CopyPaste-bdac.11).
 *
 * Extracted from SettingsActivity's private `SettingsNavRow`. Tapping the row
 * calls [onClick] to navigate elsewhere; there is no trailing control.
 * Fixed §5 comfortable spacing (density modes removed — §2/§12).
 *
 * @param title         Main label text.
 * @param subtitle      Secondary description text.
 * @param leadingIcon   Optional leading icon (e.g. NavIcons.About/Logs).
 * @param onClick       Called when the row is tapped.
 */
@Composable
fun SharedSettingsNavRow(
    title: String,
    subtitle: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    leadingIcon: androidx.compose.ui.graphics.vector.ImageVector? = null,
) {
    val c = LocalCpColors.current
    // §5 fixed comfortable spacing — density modes removed (CopyPaste-xruv, §2/§12).
    val vertPad = 12.dp
    Row(
        modifier = modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(horizontal = 16.dp, vertical = vertPad),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        if (leadingIcon != null) {
            Icon(
                imageVector = leadingIcon,
                contentDescription = null,
                tint = c.dim,
                modifier = Modifier.size(20.dp),
            )
            Spacer(modifier = Modifier.width(12.dp))
        }
        Column(modifier = Modifier
            .weight(1f)
            .padding(end = 12.dp)) {
            Text(
                text = title,
                style = MaterialTheme.typography.bodyLarge,
                color = c.text,
            )
            Text(
                text = subtitle,
                style = MaterialTheme.typography.bodySmall,
                color = c.dim,
            )
        }
    }
}

// ---------------------------------------------------------------------------
// EmptyStateCard — shared empty-state composable (CopyPaste-bdac.15).
//
// Extracted from HistoryActivity's EmptyHistoryState / EmptySearchState so
// other screens (LogViewerActivity, etc.) can use the same icon+card pattern
// without duplicating the animated card structure.
//
// Callers provide the [icon], [title], and [subtitle] strings. The animation
// (fade+scale entrance) is gated on [reducedMotion]. [iconTint] defaults to
// the theme's accent2 (same as the history empty-state icon).
// ---------------------------------------------------------------------------

/**
 * Structured empty-state card with animated entrance (CopyPaste-bdac.15).
 *
 * Renders a 58dp icon box with an accent-tinted bg + accent border, followed
 * by a title + subtitle text column inside a [CopyPasteCard]. The entrance
 * animation (fade + scale-in) is skipped when [reducedMotion] is true.
 *
 * Extracted from HistoryActivity's `EmptyHistoryState`; preserves styling.
 *
 * @param icon          Icon to display in the 58dp icon box.
 * @param title         Primary empty-state label.
 * @param subtitle      Supporting description.
 * @param padding       Content padding (from surrounding Scaffold).
 * @param iconTint      Tint for the icon; defaults to the theme's accent2.
 * @param reducedMotion When true, disables the entrance animation.
 */
@Composable
fun EmptyStateCard(
    icon: @Composable () -> Unit,
    title: String,
    subtitle: String,
    padding: PaddingValues,
    modifier: Modifier = Modifier,
    reducedMotion: Boolean = false,
) {
    val c = LocalCpColors.current
    val translucent = rememberTranslucency()
    val enterDurMs = if (reducedMotion) 0 else 400

    Box(
        modifier = modifier
            .fillMaxWidth()
            .then(Modifier.background(if (translucent) androidx.compose.ui.graphics.Color.Transparent else c.bg))
            .padding(padding)
            .padding(horizontal = 32.dp, vertical = 24.dp),
        contentAlignment = Alignment.Center,
    ) {
        AnimatedVisibility(
            visible = true,
            enter = if (reducedMotion || enterDurMs == 0)
                        EnterTransition.None
                    else
                        fadeIn(tween(enterDurMs)) + scaleIn(
                            tween(enterDurMs),
                            initialScale = 0.92f,
                        ),
        ) {
            CopyPasteCard(
                modifier = Modifier.widthIn(max = 400.dp),
                accent = MaterialTheme.colorScheme.outline,
                translucent = translucent,
            ) {
                Row(
                    modifier = Modifier.padding(horizontal = 20.dp, vertical = 20.dp),
                    horizontalArrangement = Arrangement.spacedBy(16.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    // 58dp icon box with accent-tinted bg + border (mirrors EmptyHistoryState).
                    Box(
                        modifier = Modifier
                            .size(58.dp)
                            .background(
                                color = accentFill().copy(alpha = 0.15f),
                                shape = RoundedCornerShape(20.dp),
                            )
                            .border(
                                width = 1.dp,
                                color = accentFill().copy(alpha = 0.28f),
                                shape = RoundedCornerShape(20.dp),
                            ),
                        contentAlignment = Alignment.Center,
                    ) {
                        icon()
                    }
                    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        Text(
                            text = title,
                            style = MaterialTheme.typography.bodyLarge.copy(
                                fontWeight = FontWeight.SemiBold,
                            ),
                            color = c.text,
                        )
                        Text(
                            text = subtitle,
                            style = MaterialTheme.typography.bodyMedium,
                            color = c.dim,
                        )
                    }
                }
            }
        }
    }
}

@Composable
fun CopyPasteIconButton(
    onClick: () -> Unit,
    contentDescription: String?,
    icon: @Composable () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    hitTarget: Dp = 44.dp,
) {
    val interaction = remember { MutableInteractionSource() }
    Box(
        modifier = modifier
            .size(hitTarget)
            .clip(CircleShape)
            .clickable(
                enabled = enabled,
                role = Role.Button,
                interactionSource = interaction,
                indication = null,
                onClick = onClick,
            )
            .then(if (contentDescription != null) Modifier.semantics { this.contentDescription = contentDescription } else Modifier)
            .alpha(if (enabled) 1f else 0.40f),
        contentAlignment = Alignment.Center,
    ) { icon() }
}
