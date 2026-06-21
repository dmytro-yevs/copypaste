package com.copypaste.android

import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.annotation.StringRes
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import com.copypaste.android.ui.theme.NavIcons
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.core.content.ContextCompat
import androidx.lifecycle.lifecycleScope
import com.copypaste.android.ui.SyncStatusBadge
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.GlassTier
import com.copypaste.android.ui.theme.LiquidGlassSurface
import com.copypaste.android.ui.theme.LocalIdeColors
import com.copypaste.android.ui.theme.LocalPalette
import com.copypaste.android.ui.theme.LocalSkin
import com.copypaste.android.ui.theme.SkinNavActive
import com.copypaste.android.ui.theme.skinTokens
import com.copypaste.android.ui.theme.auroraCanvas
import com.copypaste.android.ui.theme.glassFloatShadow
import com.copypaste.android.ui.theme.isDarkTheme
import com.copypaste.android.ui.theme.paletteAurora
import com.copypaste.android.ui.theme.rememberReducedMotion
import com.copypaste.android.ui.theme.rememberTranslucency
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

/**
 * Root activity — hosts the three-tab bottom navigation shell:
 *   0. Clips     (clipboard history, start destination)
 *   1. Devices   (pair a new device / relay)
 *   2. Settings
 *
 * On first launch (or when critical permissions are missing) the user is
 * forwarded to [OnboardingActivity]. On resume the permission check runs
 * again so the onboarding prompt can re-appear if the user revokes access.
 *
 * Clipboard monitoring:
 *   - [ClipboardService] covers API 26-28 (background allowed).
 *   - [LogcatCaptureService] covers API 29+ background access via the logcat+overlay path
 *     (requires READ_LOGS via adb + SYSTEM_ALERT_WINDOW).
 *   - The in-activity [ClipboardManager] listener below covers the foreground
 *     window while this activity is visible (all API levels).
 */
class MainActivity : ComponentActivity() {

    private val viewModel: ClipboardViewModel by viewModels()
    private lateinit var clipboardManager: ClipboardManager
    private lateinit var repository: ClipboardRepository
    private lateinit var settings: Settings
    private lateinit var syncManager: SyncManager

    /**
     * L10: whether the onboarding screen has already been forwarded to during
     * this Activity's lifetime. Instance-scoped (was a process-static `var`,
     * which suppressed onboarding forever after the first launch even across
     * fresh Activity instances). Persisted into savedInstanceState so a config
     * change / process-death restore does not re-trigger onboarding mid-task.
     */
    private var onboardingShownThisSession = false

    private val clipListener = ClipboardManager.OnPrimaryClipChangedListener {
        val clip = clipboardManager.primaryClip ?: return@OnPrimaryClipChangedListener

        // Image branch: check all MIME types before falling through to text.
        // M7: lifecycleScope is used here too so the coroutine is cancelled in onDestroy.
        val imageMime = (0 until clip.description.mimeTypeCount)
            .map { clip.description.getMimeType(it) }
            .firstOrNull { it.startsWith("image/") }
        if (imageMime != null) {
            val uri = clip.getItemAt(0)?.uri
            if (uri != null) {
                lifecycleScope.launch(Dispatchers.IO) {
                    ClipboardService.captureImageClip(this@MainActivity, uri, imageMime, settings, repository, syncManager)
                }
            }
            return@OnPrimaryClipChangedListener
        }

        val text = clip.getItemAt(0)?.text?.toString() ?: return@OnPrimaryClipChangedListener
        // M7: use the Activity's lifecycleScope so the coroutine is cancelled
        // automatically in onDestroy — the old hand-rolled CoroutineScope was
        // never cancelled, leaking the Activity/ViewModel via the captured `this`.
        lifecycleScope.launch(Dispatchers.IO) { handleClipboardChange(text) }
    }

    // CopyPaste-l080: request POST_NOTIFICATIONS BEFORE starting the FGS on
    // Android 13+ first launch. Whatever the result, start the service afterwards
    // so capture still works — but if granted, the FGS status notification (Pause/
    // Resume) is now actually visible instead of being silently dropped.
    private val notifLauncher = registerForActivityResult(
        androidx.activity.result.contract.ActivityResultContracts.RequestPermission()
    ) { granted ->
        Log.d(TAG, "MainActivity POST_NOTIFICATIONS granted=$granted")
        startClipboardServiceIfPossible()
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // CopyPaste-1g00: screenshot protection is now pref-driven (Settings.allowScreenshots).
        // CopyPasteTheme applies FLAG_SECURE centrally when allowScreenshots=false (the default).
        // The old hardcoded setFlags(FLAG_SECURE) is removed so the user's pref is honoured.
        applyScreenshotPolicy(Settings(this))
        // Edge-to-edge: the bottom NavigationBar and each tab's TopAppBar apply
        // their own system-bar insets so nothing is clipped on notched phones.
        enableEdgeToEdge()

        onboardingShownThisSession =
            savedInstanceState?.getBoolean(KEY_ONBOARDING_SHOWN, false) ?: false

        settings = Settings(this)
        repository = ClipboardRepository(this)
        // [P2] Wrap sync setup in try/catch so a constructor failure does not
        // prevent the clipboard listener from registering. Falls back to a stub
        // RelayClient pointing at an empty base URL; the relay cloud path is already
        // disabled (SyncBackend.RELAY is a no-op) so the listener still works.
        try {
            val relayClient = RelayClient(settings.relayUrl)
            // [P1] The relay bearer token (used by RelayClient.uploadItem) is obtained
            // from RelayClient.registerDevice() at pairing time and was never persisted
            // to Settings — there is no relayToken field on Settings. The SyncBackend.RELAY
            // cloud upload path is DISABLED (ClipboardService.notifySyncManager logs a
            // warning and returns without calling uploadItem), so token="" causes no 401s
            // in practice. If the relay path is ever re-enabled, store the Device.token
            // returned by registerDevice() in Settings and pass it here.
            syncManager = SyncManager(relayClient, settings.deviceId, token = "", settings = settings)
        } catch (e: Exception) {
            Log.w(TAG, "SyncManager init failed — proceeding without relay sync: ${e.javaClass.simpleName} ${e.message}")
            val fallback = RelayClient("")
            syncManager = SyncManager(fallback, settings.deviceId, token = "", settings = settings)
        }
        clipboardManager = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        clipboardManager.addPrimaryClipChangedListener(clipListener)

        // CopyPaste-l080: on Android 13+ first launch, request POST_NOTIFICATIONS
        // BEFORE starting the foreground service so its status notification is
        // visible (previously the FGS started first and the notification was
        // silently dropped — no Pause/Resume — until the next launch after grant).
        // The launcher callback starts the service whatever the user chooses; if
        // already granted (or pre-Tiramisu) we start it directly.
        if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.TIRAMISU &&
            !NotificationPermissionHelper.isGranted(this) &&
            !NotificationPermissionHelper.isPermanentlyDenied(this)
        ) {
            NotificationPermissionHelper.markRequested(this)
            notifLauncher.launch(android.Manifest.permission.POST_NOTIFICATIONS)
        } else {
            startClipboardServiceIfPossible()
        }

        setContent {
            CopyPasteTheme {
                MainShell(viewModel = viewModel)
            }
        }
    }

    override fun onResume() {
        super.onResume()
        // Re-check permissions on every resume; open onboarding if needed.
        if (!onboardingShownThisSession && !OnboardingActivity.allCriticalGranted(this)) {
            onboardingShownThisSession = true
            startActivity(Intent(this, OnboardingActivity::class.java))
        }
    }

    override fun onSaveInstanceState(outState: Bundle) {
        super.onSaveInstanceState(outState)
        outState.putBoolean(KEY_ONBOARDING_SHOWN, onboardingShownThisSession)
    }

    override fun onDestroy() {
        clipboardManager.removePrimaryClipChangedListener(clipListener)
        super.onDestroy()
    }

    private fun startClipboardServiceIfPossible() {
        try {
            ContextCompat.startForegroundService(this, Intent(this, ClipboardService::class.java))
            Log.d(TAG, "ClipboardService start requested")
        } catch (e: Exception) {
            Log.w(TAG, "ClipboardService start failed: ${e.javaClass.simpleName} ${e.message}")
        }
    }

    private suspend fun handleClipboardChange(text: String) {
        // Route through the shared capture pipeline so foreground-captured clips
        // are counted in the notification, trigger copy sound/notification, and
        // sync — exactly like ClipboardService and LogcatCaptureService.
        // Previously this called repository.storeItem directly, skipping all of
        // bumpTodayCounter / postCopyNotification / playCopySound / notifySyncManager.
        ClipboardService.captureClip(this, text, settings, repository, syncManager)
        // Do NOT call viewModel.loadItems() here. The ViewModel's storeListener
        // (registered on SharedPreferences KEY_ITEM_IDS) already fires a loadItems()
        // automatically whenever captureClip actually stores a new item. Calling it
        // unconditionally here caused a redundant refresh on every clipboard change —
        // including suppressed echo-copies (copy-from-history taps) — and could
        // interact with HW-A3 by triggering a UI reload before the dedup window
        // had a chance to suppress all concurrent listener fires.
    }

    companion object {
        private const val TAG = "MainActivity"
        private const val KEY_ONBOARDING_SHOWN = "onboarding_shown_this_session"
    }
}

// ── Navigation structure ───────────────────────────────────────────────────────

// Internal so NavTabTest (pure-JVM unit test) can verify the tab set.
// `labelRes` is the bottom-nav label string resource. HB-6: the DEVICES tab now
// reads R.string.title_devices ("Devices") instead of the old hardcoded "Pair",
// matching the Devices screen title — pairing lives INSIDE that screen now.
internal enum class NavTab(@StringRes val labelRes: Int, val icon: ImageVector) {
    // CopyPaste-dm51 (NavIcons.kt): bespoke SF-like thin-stroke ImageVectors
    // matching web NavIcons.tsx (clock.arrow.circlepath / laptopcomputer.and.iphone / gear).
    // Previously used Icons.Outlined.History/Hub/Settings (thicker Material icons).
    CLIPS(R.string.title_history, NavIcons.History),
    DEVICES(R.string.title_devices, NavIcons.Devices),
    SETTINGS(R.string.title_settings, NavIcons.Settings),
}

@Composable
private fun MainShell(viewModel: ClipboardViewModel) {
    var selectedTab by rememberSaveable { mutableIntStateOf(NavTab.CLIPS.ordinal) }
    // Unsaved-changes guard registered by SettingsScreen. When the user has
    // pending edits and tries to switch tabs via the navbar, we route the tab
    // change through this guard so the Discard/Keep-editing dialog intercepts it
    // (parity with the back-press / top-bar back-arrow guard). Null when not on
    // Settings or when there are no unsaved changes.
    var settingsNavGuard by remember {
        mutableStateOf<((proceed: () -> Unit) -> Unit)?>(null)
    }

    // §3 Translucency: read once at the shell level so the pref is consistent
    // across the floating tab bar and all child screens. CopyPasteTopBar and
    // CopyPasteCard read it independently via rememberTranslucency() for
    // screens rendered without MainShell (standalone activities).
    val c = LocalIdeColors.current
    val translucent = rememberTranslucency()
    val dark = isDarkTheme()
    val palette = LocalPalette.current
    val density = LocalDensity.current

    // Styleguide floating tab bar geometry:
    //   side margin 12 dp, bottom margin 10 dp (matching web `.tabbar` margin)
    //   radius 28 dp, internal padding 8 dp top/sides + 12 dp bottom
    //   height driven by content — no fixed min-height (Android wraps tightly)
    val tabBarShape = RoundedCornerShape(28.dp)
    // Bottom safe-area (nav bar) inset so the bar clears the system nav buttons.
    val navBarInsetDp = WindowInsets.navigationBars
        .asPaddingValues()
        .calculateBottomPadding()
    // Measured height of the FloatingTabBar, updated via onSizeChanged once the
    // bar lays out. 74.dp is the initial fallback (icon + label at normal density)
    // so content padding is reasonable on the first frame before measurement fires.
    // Using onSizeChanged avoids over-padding on low-density and clipping on
    // high-density / large-font-scale devices (CopyPaste-10tp item 4).
    var tabBarHeightDp by remember { mutableStateOf(74.dp) }
    val contentBottomPadding = tabBarHeightDp + 10.dp + navBarInsetDp

    // §1 aurora canvas backdrop: a COLOURED radial-glow gradient behind the whole
    // shell so the glass surfaces (tab bar, cards) frost over real colour instead
    // of a flat fill. Only when translucent — otherwise keep the opaque c.bg so the
    // solid look is unchanged. The Scaffold container goes transparent so this shows.
    Box(
        modifier = Modifier.fillMaxSize().then(
            if (translucent) Modifier.auroraCanvas(dark, paletteAurora(palette)) else Modifier
        ),
    ) {
        Scaffold(
            containerColor = if (translucent) Color.Transparent else c.bg,
            // Zero all Scaffold insets: the TOP inset is handled by each screen's own
            // TopAppBar, and the BOTTOM is handled by explicit content padding below so
            // the list clears the floating tab bar. Applying insets here would double-
            // inset the top (status-bar) on every screen that already adds it.
            contentWindowInsets = WindowInsets(0, 0, 0, 0),
        ) { innerPadding ->
            // CopyPaste-r3qq fix: use fillMaxSize instead of Column(padding(innerPadding))
            // to eliminate the grey strip that the inner padding + Column produced above
            // the floating bar. contentBottomPadding ensures the list clears the bar.
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(innerPadding)
                    .padding(bottom = contentBottomPadding),
            ) {
                when (NavTab.entries[selectedTab]) {
                    NavTab.CLIPS -> HistoryScreen(
                        viewModel = viewModel,
                        showBackButton = false,
                        onBack = {},
                        // Shell already paints the full-window aurora behind everything.
                        paintCanvasBackdrop = false,
                    )
                    NavTab.DEVICES -> DevicesScreen(
                        showBackButton = false,
                        onBack = {},
                        paintCanvasBackdrop = false,
                    )
                    NavTab.SETTINGS -> SettingsScreen(
                        showBackButton = false,
                        onBack = {},
                        onRegisterNavGuard = { guard -> settingsNavGuard = guard },
                        paintCanvasBackdrop = false,
                        // CopyPaste-u30t: navigate to the History/home tab after saving.
                        onSaved = { selectedTab = NavTab.CLIPS.ordinal },
                    )
                }
                // CopyPaste-r3qq: SyncStatusBadge overlaid at bottom-center as a Box
                // child so it floats above the screen content rather than being pushed
                // below the tab bar by Column layout. z-order: content < badge < tab bar.
                Box(modifier = Modifier.align(Alignment.BottomCenter)) {
                    SyncStatusBadge()
                }
            }
        }

        // ── Floating glass tab bar ──────────────────────────────────────────
        // Detached from the Scaffold bottomBar slot so it floats over the content
        // with side margins (12 dp) and a rounded glass pill (radius 28 dp).
        // Positioned via Alignment.BottomCenter + padding, clears the system nav bar.
        FloatingTabBar(
            modifier = Modifier
                .align(Alignment.BottomCenter)
                .onSizeChanged { size ->
                    tabBarHeightDp = with(density) { size.height.toDp() }
                },
            selectedTab = selectedTab,
            translucent = translucent,
            dark = dark,
            tabBarShape = tabBarShape,
            navBarBottomPadding = navBarInsetDp,
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

/**
 * Floating glass tab bar (styleguide `.tabbar` floating treatment).
 *
 * Detached from the screen edge — sits 10 dp above the system navigation bar,
 * with 12 dp side margins and a 28 dp corner radius. The glass surface is
 * [GlassTier.GLASS] (blur 28dp, saturate 180%, per-tier white gradient fill +
 * hairline rim). Soft float shadow (0 18dp 45dp rgb(0,0,0/.20)) sits behind it.
 *
 * Active tab: accent-tinted pill background (accent@75%) + [c.accentOn] icon/label
 * + a spring pop scale (0.94 → 1.06 → 1.0) gated by [motionDuration].
 * Inactive: [c.faint] icon/label, no background.
 */
@Composable
private fun FloatingTabBar(
    modifier: Modifier = Modifier,
    selectedTab: Int,
    translucent: Boolean,
    dark: Boolean,
    tabBarShape: RoundedCornerShape,
    navBarBottomPadding: androidx.compose.ui.unit.Dp,
    onTabSelected: (Int) -> Unit,
) {
    val c = LocalIdeColors.current
    // CopyPaste-jr5a: skin-aware active indicator for the nav tab bar.
    // Reads the skin token once per composition (staticCompositionLocalOf, stable).
    val tok = skinTokens(LocalSkin.current)
    val reducedMotion = rememberReducedMotion()
    // Spring spec for the active-tab scale pop: stiffness Low → smooth spring,
    // dampingRatio NoBouncy → one clean overshoot then settle.
    val springSpec = spring<Float>(
        dampingRatio = Spring.DampingRatioLowBouncy,
        stiffness = Spring.StiffnessMedium,
    )

    LiquidGlassSurface(
        shape = tabBarShape,
        translucent = translucent,
        dark = dark,
        solid = c.panel,
        tier = GlassTier.GLASS,
        // The hairline glass rim gives the pill its edge definition.
        hairline = true,
        // CopyPaste-r3qq: fillMaxWidth MUST come BEFORE padding so the horizontal
        // padding actually bites into the full width (previously padding came first,
        // then fillMaxWidth ignored it and expanded to full parent width).
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = 12.dp, vertical = 10.dp)
            .padding(bottom = navBarBottomPadding)
            // Styleguide: box-shadow 0 18px 45px rgba(0,0,0,.20) — soft float shadow.
            .then(if (translucent) Modifier.glassFloatShadow(GlassTier.GLASS, 28.dp) else Modifier),
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 8.dp, vertical = 8.dp)
                .padding(bottom = 4.dp),
            horizontalArrangement = Arrangement.SpaceEvenly,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            NavTab.entries.forEachIndexed { index, tab ->
                val isSelected = selectedTab == index
                val label = stringResource(tab.labelRes)

                // Spring pop on selection: 0.94 → 1.06 → 1.0 (matches web activeTabPop
                // keyframes @0%:scale(.94), @60%:scale(1.06), @100%:scale(1)).
                // Gated: reducedMotion → instant snap to 1.0f, no animation.
                val scale by animateFloatAsState(
                    targetValue = if (isSelected) 1.0f else 0.97f,
                    animationSpec = if (reducedMotion) spring(stiffness = Spring.StiffnessHigh) else springSpec,
                    label = "tabScale_$index",
                )

                // CopyPaste-jr5a: skin-aware active pill — driven by tok.navActive.
                //   FILL_GLOW  — Classic: solid accent fill, accentOn text. No ring. (byte-identical to old behaviour)
                //   TINT       — Quiet: accentDim tinted background, accent text. No ring.
                //   GLASS_RING — Vapor: elevated background + 1dp accent outline ring, accent text.
                val activePillBg = when (tok.navActive) {
                    SkinNavActive.FILL_GLOW  -> c.accent     // Classic: solid accent pill
                    SkinNavActive.TINT       -> c.accentDim  // Quiet: subtle tint
                    SkinNavActive.GLASS_RING -> c.elevated   // Vapor: elevated surface + ring
                }
                val activeIconColor = when (tok.navActive) {
                    SkinNavActive.FILL_GLOW  -> c.accentOn  // on-accent icon
                    SkinNavActive.TINT       -> c.accent    // accent-coloured icon on tint
                    SkinNavActive.GLASS_RING -> c.accent    // accent-coloured icon on glass
                }
                val iconColor = if (isSelected) activeIconColor else c.faint
                val textColor = if (isSelected) activeIconColor else c.faint
                val pillColor = if (isSelected) activePillBg else Color.Transparent
                val showRing = isSelected && tok.navActive == SkinNavActive.GLASS_RING

                Box(
                    modifier = Modifier
                        .weight(1f)
                        .scale(scale)
                        .clip(RoundedCornerShape(18.dp))
                        .background(pillColor)
                        .then(
                            // GLASS_RING: 1dp accent outline ring on the selected tab (Vapor nav spec).
                            // Classic and Quiet do not add a border — Classic is visually byte-identical.
                            if (showRing) Modifier.border(1.dp, c.accent, RoundedCornerShape(18.dp))
                            else Modifier
                        )
                        .clickable(
                            interactionSource = remember { MutableInteractionSource() },
                            indication = null,
                            role = Role.Tab,
                            onClick = { onTabSelected(index) },
                        )
                        .padding(vertical = 8.dp),
                    contentAlignment = Alignment.Center,
                ) {
                    Column(
                        horizontalAlignment = Alignment.CenterHorizontally,
                        verticalArrangement = Arrangement.spacedBy(3.dp),
                    ) {
                        Icon(
                            imageVector = tab.icon,
                            // CopyPaste-n7ff: null — the visible label already names the tab;
                            // describing the icon too makes TalkBack announce the name twice.
                            contentDescription = null,
                            tint = iconColor,
                            modifier = Modifier.size(22.dp),
                        )
                        Text(
                            text = label,
                            color = textColor,
                            style = MaterialTheme.typography.labelSmall.copy(
                                fontSize = 10.sp,
                                fontWeight = FontWeight.W700,
                                letterSpacing = 0.sp,
                            ),
                        )
                    }
                }
            }
        }
    }
}
