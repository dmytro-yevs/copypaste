package com.copypaste.android.ui.shell

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.Dp
import com.copypaste.android.ui.SyncStatusBadge
import com.copypaste.android.ui.theme.BlurMode

// ---------------------------------------------------------------------------
// MainShellContent — hermetic mirror of [MainShell]'s Box/Scaffold/NavPill/
// NavGradientFade/SyncStatusBadge layout tree (f0f3a.8, precedent:
// HistoryListBody's extraction from HistoryScreen). Takes only plain data,
// callbacks, and three content slots — no ClipboardViewModel, no repository,
// no FFI — so it can be Paparazzi-snapshotted directly.
//
// Intentionally NOT hermetic-adjacent state that stays owned by [MainShell]
// and is NOT threaded through here: `settingsNavGuard` resolution and
// `onSaved` tab-switch-on-save (both real ViewModel/Settings-screen
// orchestration), and the ViewModel itself. [MainShell] still builds the
// three content-slot lambdas that close over its ViewModel and passes them
// in; this composable only lays out whatever it is given.
// ---------------------------------------------------------------------------

@Composable
internal fun MainShellContent(
    selectedTab: NavTab,
    onTabSelected: (NavTab) -> Unit,
    blurMode: BlurMode,
    reducedMotion: Boolean,
    imeVisible: Boolean,
    sideOffset: Dp,
    bottomOffset: Dp,
    pillHeightDp: Dp,
    onPillHeightChanged: (heightPx: Int) -> Unit,
    backdropState: BackdropCaptureState?,
    clipsContent: @Composable (bottomContentPadding: Dp) -> Unit,
    devicesContent: @Composable () -> Unit,
    settingsContent: @Composable () -> Unit,
) {
    val reservedBottomSpace = bottomOffset + pillHeightDp

    Box(modifier = Modifier.fillMaxSize()) {
        Box(modifier = Modifier.fillMaxSize().captureBackdrop(backdropState)) {
            Scaffold(
                containerColor = MaterialTheme.colorScheme.background,
                contentWindowInsets = WindowInsets(0, 0, 0, 0),
            ) { innerPadding ->
                when (selectedTab) {
                    NavTab.CLIPS -> Box(modifier = Modifier.fillMaxSize().padding(innerPadding)) {
                        clipsContent(reservedBottomSpace)
                    }
                    NavTab.DEVICES -> Box(
                        modifier = Modifier
                            .fillMaxSize()
                            .padding(innerPadding)
                            .padding(bottom = reservedBottomSpace),
                    ) {
                        devicesContent()
                    }
                    NavTab.SETTINGS -> Box(
                        modifier = Modifier
                            .fillMaxSize()
                            .padding(innerPadding)
                            .padding(bottom = reservedBottomSpace),
                    ) {
                        settingsContent()
                    }
                }
            }

            NavGradientFade(
                modifier = Modifier.align(Alignment.BottomCenter).fillMaxWidth(),
                height = reservedBottomSpace,
            )

            Box(
                modifier = Modifier
                    .align(Alignment.BottomCenter)
                    .padding(bottom = reservedBottomSpace),
            ) {
                SyncStatusBadge()
            }
        }

        NavPill(
            tabs = remember { NavTab.entries.map { NavPillTab(it.labelRes, it.icon) } },
            selectedIndex = selectedTab.ordinal,
            blurMode = blurMode,
            reducedMotion = reducedMotion,
            visible = !imeVisible,
            sideOffset = sideOffset,
            bottomOffset = bottomOffset,
            backdropState = backdropState,
            modifier = Modifier.align(Alignment.BottomCenter),
            onPillHeightChanged = onPillHeightChanged,
            onTabSelected = { index -> onTabSelected(NavTab.entries[index]) },
        )
    }
}
