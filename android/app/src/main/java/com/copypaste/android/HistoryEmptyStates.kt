package com.copypaste.android

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.scaleIn
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.animation.core.FastOutSlowInEasing
import com.copypaste.android.ui.theme.CopyPasteCard

// ─────────────────────────────────────────────────────────────────────────────
// Loading state
// ─────────────────────────────────────────────────────────────────────────────

@Composable
internal fun LoadingBox(padding: PaddingValues) {
    val c = MaterialTheme.colorScheme
    Box(
        // g5u1: dropped the redundant .background(c.background) — the Scaffold
        // already paints containerColor behind this content.
        modifier = Modifier
            .fillMaxSize()
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
        // g5u1: dropped the redundant .background(c.background) — the Scaffold
        // already paints containerColor behind this content.
        modifier = Modifier
            .fillMaxSize()
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
                accent = MaterialTheme.colorScheme.outline, // neutral border, not semantic
            ) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    // Icon container removed (de-style pass) — text-only empty state.
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
        // g5u1: dropped the redundant .background(c.background) — the Scaffold
        // already paints containerColor behind this content.
        modifier = Modifier
            .fillMaxSize()
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
                accent = MaterialTheme.colorScheme.outline,
            ) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    // Icon container removed (de-style pass) — text-only empty state.
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
