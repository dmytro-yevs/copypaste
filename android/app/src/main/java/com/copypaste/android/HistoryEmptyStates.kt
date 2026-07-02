package com.copypaste.android

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.scaleIn
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.wrapContentWidth
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.animation.core.FastOutSlowInEasing
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.CopyPasteCard
import com.copypaste.android.ui.theme.CpTypography
import com.copypaste.android.ui.theme.LocalCpColors
import com.copypaste.android.ui.theme.icons.LucideIcons

// ─────────────────────────────────────────────────────────────────────────────
// Loading state
// ─────────────────────────────────────────────────────────────────────────────

@Composable
internal fun LoadingBox(padding: PaddingValues) {
    val c = MaterialTheme.colorScheme
    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(c.background)
            .padding(padding),
        contentAlignment = Alignment.Center,
    ) {
        CircularProgressIndicator(color = c.primary)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// §9 Empty states — hero icon + title + sentence.
// Matches desktop HistoryView empty pattern exactly.
// ─────────────────────────────────────────────────────────────────────────────

/** §9 Empty state: history is empty — "Nothing copied yet".
 *
 * Styleguide .empty-state / .empty-icon (L937–979):
 *   - Grid layout: icon box + text column.
 *   - Icon box: radial-gradient bg (accent@15%) + accent border + pulsing ring halo.
 *   - Entrance: fade + scale-in, 450ms duration, FastOutSlowInEasing.
 */
@Composable
internal fun EmptyHistoryState(padding: PaddingValues, isPrivateMode: Boolean = false) {
    val c = MaterialTheme.colorScheme
    val enterDurMs = 450

    // Halo ring removed — idle pulse animation was distracting; static border below.

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(c.background)
            .padding(padding),
        contentAlignment = Alignment.Center,
    ) {
        AnimatedVisibility(
            visible = true,
            enter = fadeIn(tween(enterDurMs, easing = FastOutSlowInEasing)) +
                         scaleIn(
                             tween(enterDurMs, easing = FastOutSlowInEasing),
                             initialScale = 0.92f,
                         ),
        ) {
            CopyPasteCard(
                modifier = Modifier.widthIn(max = 400.dp),
                accent = MaterialTheme.colorScheme.outline, // neutral border, not semantic
            ) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    // Icon container: accent@15% bg with gradient shimmer border.
                    Box(
                        modifier = Modifier
                            .background(
                                color = c.primary.copy(alpha = 0.15f),
                                shape = RoundedCornerShape(20.dp),
                            )
                            .border(
                                width = 1.dp,
                                color = c.primary.copy(alpha = 0.28f),
                                shape = RoundedCornerShape(20.dp),
                            ),
                        contentAlignment = Alignment.Center,
                    ) {}
                    Column {
                        // CopyPaste-crh3.31: tailor the empty state to private mode
                        // (parity with macOS HistoryView), so the user is not misled
                        // into thinking nothing was ever copied.
                        Text(
                            text = stringResource(
                                if (isPrivateMode) R.string.empty_history_private
                                else R.string.empty_history,
                            ),
                            color = c.onSurface,
                        )
                        Text(
                            text = stringResource(
                                if (isPrivateMode) R.string.empty_history_private_subtitle
                                else R.string.empty_history_subtitle,
                            ),
                            color = c.onSurfaceVariant,
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
    val c = MaterialTheme.colorScheme
    val enterDurMs = 450

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(c.background)
            .padding(padding),
        contentAlignment = Alignment.Center,
    ) {
        AnimatedVisibility(
            visible = true,
            enter = fadeIn(tween(enterDurMs, easing = FastOutSlowInEasing)) +
                         scaleIn(
                             tween(enterDurMs, easing = FastOutSlowInEasing),
                             initialScale = 0.92f,
                         ),
        ) {
            CopyPasteCard(
                modifier = Modifier.widthIn(max = 400.dp),
                accent = MaterialTheme.colorScheme.outline,
            ) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    // Icon container: accent@12% bg, no halo for search-empty.
                    Box(
                        modifier = Modifier
                            .background(
                                color = c.primary.copy(alpha = 0.12f),
                                shape = RoundedCornerShape(20.dp),
                            )
                            .border(
                                width = 1.dp,
                                color = c.primary.copy(alpha = 0.24f),
                                shape = RoundedCornerShape(20.dp),
                            ),
                        contentAlignment = Alignment.Center,
                    ) {}
                    Column {
                        Text(
                            text = stringResource(R.string.empty_search_title, query),
                            color = c.onSurface,
                        )
                        Text(
                            text = stringResource(R.string.empty_search_subtitle),
                            color = c.onSurfaceVariant,
                        )
                    }
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// android-history 5.3 — error/degraded state (NEW). spec.md "List Display
// States": "a persistent error/degraded state is shown in the list surface
// itself" and "the error is not communicated solely via a transient toast".
// Distinct from [EmptyHistoryState]/[EmptySearchState] both in copy (this is a
// failure, not "nothing here yet") and in offering an explicit retry action
// wired straight to `viewModel.loadItems()` (no new repository/IPC surface).
// ─────────────────────────────────────────────────────────────────────────────

@Composable
internal fun HistoryErrorState(padding: PaddingValues, onRetry: () -> Unit) {
    val cp = LocalCpColors.current
    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(cp.bg)
            .padding(padding),
        contentAlignment = Alignment.Center,
    ) {
        CopyPasteCard(
            modifier = Modifier.widthIn(max = 400.dp),
            accent = cp.err.copy(alpha = 0.4f),
        ) {
            Column(
                modifier = Modifier.padding(horizontal = 20.dp, vertical = 20.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Icon(
                    imageVector = LucideIcons.StatusErr,
                    contentDescription = null,
                    tint = cp.err,
                    modifier = Modifier.size(28.dp),
                )
                Text(
                    text = stringResource(R.string.history_error_title),
                    color = cp.text,
                    style = CpTypography.body,
                    textAlign = TextAlign.Center,
                    modifier = Modifier.padding(top = 10.dp),
                )
                Text(
                    text = stringResource(R.string.history_error_subtitle),
                    color = cp.faint,
                    style = CpTypography.meta,
                    textAlign = TextAlign.Center,
                    modifier = Modifier.padding(top = 4.dp),
                )
                CopyPasteButton(
                    onClick = onRetry,
                    variant = ButtonVariant.SECONDARY,
                    modifier = Modifier
                        .wrapContentWidth()
                        .padding(top = 14.dp),
                ) {
                    Text(text = stringResource(R.string.history_error_retry))
                }
            }
        }
    }
}
