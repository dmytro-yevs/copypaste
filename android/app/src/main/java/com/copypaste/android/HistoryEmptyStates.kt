package com.copypaste.android

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.scaleIn
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.ContentCopy
import androidx.compose.material.icons.outlined.SearchOff
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.copypaste.android.ui.theme.CopyPasteCard
import com.copypaste.android.ui.theme.EaseOutExpo
import com.copypaste.android.ui.theme.LocalAccent
import com.copypaste.android.ui.theme.LocalIdeColors
import com.copypaste.android.ui.theme.Motion
import com.copypaste.android.ui.theme.motionDuration
import com.copypaste.android.ui.theme.rememberReducedMotion
import com.copypaste.android.ui.theme.rememberTranslucency

// ─────────────────────────────────────────────────────────────────────────────
// Loading state
// ─────────────────────────────────────────────────────────────────────────────

@Composable
internal fun LoadingBox(padding: PaddingValues) {
    val c = LocalIdeColors.current
    val translucent = rememberTranslucency()
    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(if (translucent) Color.Transparent else c.bg)
            .padding(padding),
        contentAlignment = Alignment.Center,
    ) {
        CircularProgressIndicator(
            color = c.accent,
            strokeWidth = 2.dp,
            modifier = Modifier.size(20.dp),
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// §9 Empty states — hero icon (28dp) + title (13sp dim) + sentence (11sp faint)
// Matches desktop HistoryView empty pattern exactly.
// ─────────────────────────────────────────────────────────────────────────────

/** §9 Empty state: history is empty — clipboard icon + "Nothing copied yet".
 *
 * Styleguide .empty-state / .empty-icon (L937–979):
 *   - Grid layout: 58dp icon + text column.
 *   - Icon box: radial-gradient bg (accent@15%) + accent border + pulsing ring halo.
 *   - Entrance: fade + scale-in, motionDuration(Motion.Slow), EaseOutExpo.
 */
@Composable
internal fun EmptyHistoryState(padding: PaddingValues, isPrivateMode: Boolean = false) {
    val c = LocalIdeColors.current
    val translucent = rememberTranslucency()
    val reducedMotion = rememberReducedMotion()
    val enterDurMs = motionDuration(Motion.Slow)

    // Halo ring removed — idle pulse animation was distracting; static border below.

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(if (translucent) Color.Transparent else c.bg)
            .padding(padding)
            .padding(horizontal = 32.dp, vertical = 24.dp),
        contentAlignment = Alignment.Center,
    ) {
        AnimatedVisibility(
            visible = true,
            enter = if (reducedMotion) androidx.compose.animation.EnterTransition.None
                    else fadeIn(tween(enterDurMs, easing = EaseOutExpo)) +
                         scaleIn(
                             tween(enterDurMs, easing = EaseOutExpo),
                             initialScale = 0.92f,
                         ),
        ) {
            CopyPasteCard(
                modifier = Modifier.widthIn(max = 400.dp),
                accent = MaterialTheme.colorScheme.outline, // neutral border, not semantic
                translucent = translucent,
            ) {
                Row(
                    modifier = Modifier.padding(horizontal = 20.dp, vertical = 20.dp),
                    horizontalArrangement = Arrangement.spacedBy(16.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    // Icon box: 58dp, accent-tinted bg, with accent2 halo ring.
                    Box(
                        modifier = Modifier.size(58.dp),
                        contentAlignment = Alignment.Center,
                    ) {
                        // Icon container: accent@15% bg with gradient shimmer border.
                        Box(
                            modifier = Modifier
                                .size(58.dp)
                                .background(
                                    color = c.accent.copy(alpha = 0.15f),
                                    shape = RoundedCornerShape(20.dp),
                                )
                                .border(
                                    width = 1.dp,
                                    color = c.accent.copy(alpha = 0.28f),
                                    shape = RoundedCornerShape(20.dp),
                                ),
                            contentAlignment = Alignment.Center,
                        ) {
                            Icon(
                                imageVector = Icons.Outlined.ContentCopy,
                                contentDescription = null,
                                tint = LocalAccent.current.variant,
                                modifier = Modifier.size(26.dp),
                            )
                        }
                    }
                    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        // CopyPaste-crh3.31: tailor the empty state to private mode
                        // (parity with macOS HistoryView), so the user is not misled
                        // into thinking nothing was ever copied.
                        Text(
                            text = stringResource(
                                if (isPrivateMode) R.string.empty_history_private
                                else R.string.empty_history,
                            ),
                            style = MaterialTheme.typography.bodyLarge.copy(fontWeight = FontWeight.SemiBold),
                            color = c.text,
                        )
                        Text(
                            text = stringResource(
                                if (isPrivateMode) R.string.empty_history_private_subtitle
                                else R.string.empty_history_subtitle,
                            ),
                            style = MaterialTheme.typography.bodyMedium,
                            color = c.dim,
                        )
                    }
                }
            }
        }
    }
}

/** §9 Empty state: search returned no results. */
@Composable
internal fun EmptySearchState(padding: PaddingValues, query: String) {
    val c = LocalIdeColors.current
    val translucent = rememberTranslucency()
    val reducedMotion = rememberReducedMotion()
    val enterDurMs = motionDuration(Motion.Slow)

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(if (translucent) Color.Transparent else c.bg)
            .padding(padding)
            .padding(horizontal = 32.dp, vertical = 24.dp),
        contentAlignment = Alignment.Center,
    ) {
        AnimatedVisibility(
            visible = true,
            enter = if (reducedMotion) androidx.compose.animation.EnterTransition.None
                    else fadeIn(tween(enterDurMs, easing = EaseOutExpo)) +
                         scaleIn(
                             tween(enterDurMs, easing = EaseOutExpo),
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
                    // Icon box: 58dp, accent-tinted bg, no halo for search-empty.
                    Box(
                        modifier = Modifier
                            .size(58.dp)
                            .background(
                                color = c.accent.copy(alpha = 0.12f),
                                shape = RoundedCornerShape(20.dp),
                            )
                            .border(
                                width = 1.dp,
                                color = c.accent.copy(alpha = 0.24f),
                                shape = RoundedCornerShape(20.dp),
                            ),
                        contentAlignment = Alignment.Center,
                    ) {
                        Icon(
                            imageVector = Icons.Outlined.SearchOff,
                            contentDescription = null,
                            tint = LocalAccent.current.variant,
                            modifier = Modifier.size(24.dp),
                        )
                    }
                    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        Text(
                            text = stringResource(R.string.empty_search_title, query),
                            style = MaterialTheme.typography.bodyLarge.copy(fontWeight = FontWeight.SemiBold),
                            color = c.text,
                        )
                        Text(
                            text = stringResource(R.string.empty_search_subtitle),
                            style = MaterialTheme.typography.bodyMedium,
                            color = c.dim,
                        )
                    }
                }
            }
        }
    }
}
