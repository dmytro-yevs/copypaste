package com.copypaste.android.ui.theme

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import com.copypaste.android.ui.theme.icons.LucideIcons

// ---------------------------------------------------------------------------
// Banner — STYLEGUIDE §9.8: full-width strip, appears only when actionable.
// [icon] message (problem + fix) [action(s)], vertically centered. New in S2
// (component-inventory.md "Banner (shared) — New, replaces ad-hoc Cards in
// SyncTab"); producers migrate onto this in S11 (feedback-states) — S2 only
// establishes the token-driven component itself.
//
// Color is never the sole signal (android-iconography "Icons render only
// through token colors" + STYLEGUIDE §7 accessibility): every variant pairs
// its tint with a distinct Lucide glyph, not color alone.
// ---------------------------------------------------------------------------

/** STYLEGUIDE §9.8 banner variants — tint + glyph pairing (never color alone). */
enum class BannerVariant { WARN, ERROR, INFO, SUCCESS }

private fun BannerVariant.tint(cp: CpColors): Color = when (this) {
    BannerVariant.WARN -> cp.warn
    BannerVariant.ERROR -> cp.err
    BannerVariant.INFO -> cp.info
    BannerVariant.SUCCESS -> cp.ok
}

private fun BannerVariant.glyph(): ImageVector = when (this) {
    BannerVariant.WARN -> LucideIcons.StatusWarn
    BannerVariant.ERROR -> LucideIcons.StatusErr
    BannerVariant.INFO -> LucideIcons.StatusInfo
    BannerVariant.SUCCESS -> LucideIcons.StatusOk
}

/**
 * A single actionable banner: `[icon] message [action(s)]`. Callers own
 * dismiss-ability (STYLEGUIDE: "dismissible only where it's safe to ignore")
 * by conditionally omitting [actions] / not rendering the banner at all —
 * this composable has no built-in auto-dismiss timer.
 */
@Composable
fun CpBanner(
    message: String,
    variant: BannerVariant,
    modifier: Modifier = Modifier,
    actions: @Composable () -> Unit = {},
) {
    val cp = LocalCpColors.current
    val tint = variant.tint(cp)
    Row(
        modifier = modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(CpShapes.chip))
            .background(tint.copy(alpha = 0.12f))
            .padding(horizontal = CpSpacing.s6, vertical = CpSpacing.s5),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(CpSpacing.s5),
    ) {
        Icon(
            imageVector = variant.glyph(),
            contentDescription = null,
            tint = tint,
            modifier = Modifier.size(CpDimensions.iconMeta),
        )
        Text(
            text = message,
            style = CpTypography.body,
            color = cp.text,
            modifier = Modifier.weight(1f),
        )
        actions()
    }
}
