@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)

package com.copypaste.android.ui.shell

import android.os.Build
import androidx.annotation.StringRes
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.calculateEndPadding
import androidx.compose.foundation.layout.calculateStartPadding
import androidx.compose.foundation.layout.displayCutout
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.isImeVisible
import androidx.compose.foundation.layout.mandatorySystemGestures
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.systemBars
import androidx.compose.foundation.layout.union
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.unit.dp
import com.copypaste.android.ClipboardViewModel
import com.copypaste.android.DevicesScreen
import com.copypaste.android.HistoryScreen
import com.copypaste.android.R
import com.copypaste.android.SettingsScreen
import com.copypaste.android.ui.SyncStatusBadge
import com.copypaste.android.ui.theme.BlurMode
import com.copypaste.android.ui.theme.CpDimensions
import com.copypaste.android.ui.theme.icons.LucideIcons
import com.copypaste.android.ui.theme.rememberCpMotionReduced
import com.copypaste.android.ui.theme.rememberResolvedBlurMode

// ---------------------------------------------------------------------------
// MainShell — the app shell hosting the three-tab bottom navigation
// (android-navigation-chrome "Shell hosts three tabs" requirement). Extracted
// from MainActivity (S4 task 4.1) so it's a plain, reusable/previewable
// composable; the floating pill itself lives in [NavPill] (hermetic — see
// its kdoc). MainShell is NOT itself hermetic (it hosts the real
// ViewModel-backed screens), only the pill sub-component is.
// ---------------------------------------------------------------------------

/**
 * The three bottom-nav destinations (android-navigation-chrome "exactly three
 * tabs — Clips, Devices, Settings"). `labelRes` is the tab's string resource;
 * `icon` is its Lucide glyph (cross-platform-parity.md history/monitor-
 * smartphone/settings-2 mapping, `LucideIcons.NavHistory/NavDevices/NavSettings`).
 * See `NavTabTest` — asserted canonical elsewhere (`GeneralTab.kt`'s About/Logs
 * rows deliberately do NOT get a fourth/fifth tab; they route via Settings).
 */
internal enum class NavTab(@StringRes val labelRes: Int) {
    CLIPS(R.string.title_history) {
        override val icon get() = LucideIcons.NavHistory
    },
    DEVICES(R.string.title_devices) {
        override val icon get() = LucideIcons.NavDevices
    },
    SETTINGS(R.string.title_settings) {
        override val icon get() = LucideIcons.NavSettings
    },
    ;

    abstract val icon: ImageVector
}

@Composable
fun MainShell(viewModel: ClipboardViewModel) {
    var selectedTab by rememberSaveable { mutableIntStateOf(NavTab.CLIPS.ordinal) }
    // Unsaved-changes guard registered by SettingsScreen. When the user has
    // pending edits and tries to switch tabs via the navbar, we route the tab
    // change through this guard so the Discard/Keep-editing dialog intercepts it
    // (parity with the back-press / top-bar back-arrow guard). Null when not on
    // Settings or when there are no unsaved changes.
    var settingsNavGuard by remember {
        mutableStateOf<((proceed: () -> Unit) -> Unit)?>(null)
    }

    val density = LocalDensity.current
    val layoutDirection = LocalLayoutDirection.current
    val reducedMotion = rememberCpMotionReduced()
    val blurMode = rememberResolvedBlurMode()
    val imeVisible = WindowInsets.isImeVisible

    // android-navigation-chrome "Default placement": 12dp above the RESOLVED
    // bottom system-bar/gesture inset (nav bar + gesture handle + cutout), NOT
    // just navigationBars alone — union covers all three per the spec scenario.
    val resolvedBottomInset = WindowInsets.systemBars
        .union(WindowInsets.mandatorySystemGestures)
        .union(WindowInsets.displayCutout)
        .asPaddingValues()
        .calculateBottomPadding()
    val cutoutStart = WindowInsets.displayCutout.asPaddingValues().calculateStartPadding(layoutDirection)
    val cutoutEnd = WindowInsets.displayCutout.asPaddingValues().calculateEndPadding(layoutDirection)
    val sideOffset = maxOf(CpDimensions.navSideInset, cutoutStart, cutoutEnd)
    val bottomOffset = CpDimensions.navBottomClearance + resolvedBottomInset

    // Measured pill height (content-driven, no fixed min-height — matches the
    // pre-extraction FloatingTabBar's sizing). 74dp is the pre-measurement
    // fallback so the gradient fade / sync-status gap is reasonable on frame 1.
    // S4 review fix: this is now reported by NavPill's `onPillHeightChanged`
    // off its INNER pill box only — the outer wrapper already carries
    // [bottomOffset] as its own padding, so measuring the outer box here would
    // double-count it into [reservedBottomSpace] below.
    var pillHeightDp by remember { mutableStateOf(74.dp) }
    val reservedBottomSpace = bottomOffset + pillHeightDp

    // D7 real backdrop-capture source (see `BackdropCapture.kt`). Only ever
    // constructed when blur is actually enabled — the blur itself (RenderEffect)
    // is API 31+ regardless of the capture mechanism (see `NavPill.realBackdrop`).
    val backdropState = remember(blurMode) {
        if (blurMode == BlurMode.REAL_BACKDROP && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            BackdropCaptureState()
        } else {
            null
        }
    }

    Box(modifier = Modifier.fillMaxSize()) {
        // Capture source: everything the pill floats over (screen content,
        // gradient fade, sync badge) — recorded into [backdropState] once per
        // frame and replayed here unchanged; NavPill (outside this Box) draws
        // its own translated/blurred copy. `captureBackdrop(null)` is a no-op.
        Box(modifier = Modifier.fillMaxSize().captureBackdrop(backdropState)) {
            Scaffold(
                containerColor = MaterialTheme.colorScheme.background,
                // Zero all Scaffold insets: the TOP inset is handled by each screen's own
                // TopAppBar, and the BOTTOM is handled by explicit content padding below so
                // the list clears the floating pill.
                contentWindowInsets = WindowInsets(0, 0, 0, 0),
            ) { innerPadding ->
                // D7 edge-to-edge backdrop (S5 carried task): CLIPS renders with NO
                // outer bottom padding — its own list threads [reservedBottomSpace]
                // down as `HistoryScreen.bottomContentPadding`, which HistoryList
                // applies as LazyColumn `contentPadding` (not a Modifier.padding
                // that would shrink the composable's own drawable bounds). That
                // means History's real content pixels are laid out and scroll BEHIND
                // the floating pill, so the pill's backdrop blur samples real
                // content instead of an empty gradient-only gap.
                //
                // DEVICES/SETTINGS are NOT owned by this wave/slice (S7/S9) and keep
                // the PRE-EXISTING behaviour verbatim: an outer `Modifier.padding`
                // that shrinks their bounds above the reserved space. Adopting the
                // same contentPadding-based pattern for those two screens is tracked
                // as a follow-up for their owning slices (CopyPaste-myh8.5 bd notes).
                when (NavTab.entries[selectedTab]) {
                    NavTab.CLIPS -> Box(modifier = Modifier.fillMaxSize().padding(innerPadding)) {
                        HistoryScreen(
                            viewModel = viewModel,
                            showBackButton = false,
                            onBack = {},
                            paintCanvasBackdrop = false,
                            bottomContentPadding = reservedBottomSpace,
                        )
                    }
                    NavTab.DEVICES -> Box(
                        modifier = Modifier
                            .fillMaxSize()
                            .padding(innerPadding)
                            .padding(bottom = reservedBottomSpace),
                    ) {
                        DevicesScreen(
                            showBackButton = false,
                            onBack = {},
                            paintCanvasBackdrop = false,
                        )
                    }
                    NavTab.SETTINGS -> Box(
                        modifier = Modifier
                            .fillMaxSize()
                            .padding(innerPadding)
                            .padding(bottom = reservedBottomSpace),
                    ) {
                        SettingsScreen(
                            showBackButton = false,
                            onBack = {},
                            onRegisterNavGuard = { guard -> settingsNavGuard = guard },
                            paintCanvasBackdrop = false,
                            onSaved = { selectedTab = NavTab.CLIPS.ordinal },
                        )
                    }
                }
            }

            // ── Gradient fade + sync status, in the reserved bottom gap ──
            NavGradientFade(
                modifier = Modifier.align(Alignment.BottomCenter).fillMaxWidth(),
                height = reservedBottomSpace,
            )

            // android-navigation-chrome "Sync Status Indicator Placement": a
            // shell-owned position that never overlaps the pill — sits directly
            // above the pill's measured footprint, within the reserved gap.
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
            selectedIndex = selectedTab,
            blurMode = blurMode,
            reducedMotion = reducedMotion,
            visible = !imeVisible,
            sideOffset = sideOffset,
            bottomOffset = bottomOffset,
            backdropState = backdropState,
            modifier = Modifier.align(Alignment.BottomCenter),
            onPillHeightChanged = { heightPx ->
                pillHeightDp = with(density) { heightPx.toDp() }
            },
            onTabSelected = { index ->
                val leavingSettings =
                    NavTab.entries[selectedTab] == NavTab.SETTINGS && index != selectedTab
                val guard = settingsNavGuard
                if (leavingSettings && guard != null) {
                    // Intercept: the guard shows the Discard dialog and
                    // only runs `proceed` if the user confirms (or there
                    // are no unsaved changes).
                    guard { selectedTab = index }
                } else {
                    selectedTab = index
                }
            },
        )
    }
}
