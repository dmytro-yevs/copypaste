package com.copypaste.android.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalContext
import com.copypaste.android.Settings
import com.copypaste.android.ThemeMode
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

// ---------------------------------------------------------------------------
// Application-scoped committed appearance (design.md D5).
//
// [CommittedAppearance] is the state a Settings Save publishes; [AppearanceStore]
// holds it as a process-wide `StateFlow` (same pattern as `DevicesOnlineState`)
// so every currently-composed AND every future Activity that reads it via
// [CommittedCopyPasteTheme] re-themes without `Activity.recreate()`, which only
// recreates the current Activity instance and cannot reach stopped
// back-stack/other-task Activities (design.md R6).
//
// The Display tab's live-preview DRAFT is a separate, purely local Compose
// `mutableStateOf` scoped to the Settings screen's own composition — it feeds
// a nested [CopyPasteTheme] call directly and NEVER touches [AppearanceStore]
// except via an explicit [AppearanceStore.publish] call on Save (D5/R17: "the
// application-scoped state diverges from a failed preference commit" is
// mitigated by publishing only after `Settings.saveScreenSettings` returns
// `true`). This file intentionally has no draft-state helpers — those live
// with the Settings screen that owns the draft (SettingsActivity.kt).
// ---------------------------------------------------------------------------

/** The four appearance axes as of the last successful Settings Save (D4). */
data class CommittedAppearance(
    val themeMode: ThemeMode,
    val accent: AccentColor,
    val translucency: Boolean,
)

/**
 * Process-wide committed-appearance holder. `init` seeds the flow from
 * persisted [Settings] exactly once per process (idempotent — a later Activity
 * calling `init` again after another has already published an in-session
 * change must NOT stomp it back to the on-disk snapshot); `publish` is the
 * ONLY way a new value reaches every reader, and MUST only be called after a
 * successful `saveScreenSettings` commit (D5/R17).
 */
object AppearanceStore {
    private val _committed = MutableStateFlow(
        CommittedAppearance(ThemeMode.DEFAULT, AccentColor.DEFAULT, translucency = true),
    )

    /** Read-only stream every [CommittedCopyPasteTheme] call site collects. */
    val committed: StateFlow<CommittedAppearance> = _committed.asStateFlow()

    @Volatile
    private var initialized = false

    /**
     * Seed [committed] from [settings] once per process. MUST run after
     * `Settings.migrateThemeForTwoAxis()` (D6 ordering — see
     * `CopyPasteApp.onCreate`) so a pre-migration read never observes a stale
     * key. Safe to call repeatedly (e.g. defensively from every
     * [CommittedCopyPasteTheme] composition) — only the first call has effect.
     */
    @Synchronized
    fun init(settings: Settings) {
        if (initialized) return
        initialized = true
        _committed.value = committedAppearanceFrom(settings)
    }

    /**
     * Publish a newly-committed appearance app-wide. Callers MUST only invoke
     * this after `Settings.saveScreenSettings(...)` returns `true` (D5/R17) —
     * never from the Display tab's live-preview draft. `StateFlow` conflates
     * equal values, so publishing an unchanged [CommittedAppearance] is a
     * no-op (android-appearance "No app-wide change when unchanged").
     */
    fun publish(appearance: CommittedAppearance) {
        _committed.value = appearance
    }
}

/**
 * Reads the three appearance axes off [settings] into a [CommittedAppearance].
 * Extracted as a pure(-ish; SharedPreferences reads only) function, separate
 * from [AppearanceStore]'s "only once per process" singleton state, so the
 * Settings->CommittedAppearance mapping itself is unit-testable without
 * fighting the singleton's process-wide, run-order-sensitive `initialized`
 * flag (a real limitation of the `object` pattern this file otherwise shares
 * with `DevicesOnlineState`).
 */
internal fun committedAppearanceFrom(settings: Settings): CommittedAppearance =
    CommittedAppearance(settings.themeMode, settings.accent, settings.translucency)

/**
 * Resolves [themeMode] to the boolean [CopyPasteTheme] consumes. Pure
 * overload (no Compose dependency) for unit tests — mirrors
 * [systemBarsAreLight]'s "pure function, thin Composable wrapper" split.
 */
internal fun resolveIsDark(themeMode: ThemeMode, systemInDark: Boolean): Boolean = when (themeMode) {
    ThemeMode.DARK -> true
    ThemeMode.LIGHT -> false
    ThemeMode.SYSTEM -> systemInDark
}

/** [resolveIsDark] wired to the live OS dark-theme signal. */
@Composable
fun resolveIsDark(themeMode: ThemeMode): Boolean = resolveIsDark(themeMode, isSystemInDarkTheme())

/**
 * The committed (last-saved) app theme — wraps [CopyPasteTheme] with
 * [AppearanceStore]'s current value, resolved through [resolveIsDark]. This is
 * the composable every Activity root wraps its content in (design.md D5:
 * "embedded SettingsScreen (MainActivity tab) and standalone SettingsActivity
 * both propagate via the shared committed state"); a nested [CopyPasteTheme]
 * call fed by a LOCAL draft (SettingsScreen only) shadows this for live
 * preview without ever mutating [AppearanceStore].
 */
@Composable
fun CommittedCopyPasteTheme(content: @Composable () -> Unit) {
    val context = LocalContext.current
    val settings = remember(context) { Settings(context) }
    // Composition-time (not effect-scheduled) seed so the very first frame
    // already reflects the persisted value — an effect-scheduled init would
    // render one frame of hardcoded defaults first (D16 "no wrong-theme
    // flash" applies to the same class of problem here). init() is idempotent
    // (a Kotlin function call, not a `remember{}` — remember must not return
    // Unit, androidx.compose.runtime.RememberReturnType), so calling it on
    // every recomposition is a cheap no-op after the first.
    AppearanceStore.init(settings)
    val appearance by AppearanceStore.committed.collectAsState()
    CopyPasteTheme(
        isDark = resolveIsDark(appearance.themeMode),
        accent = appearance.accent,
        translucency = appearance.translucency,
        content = content,
    )
}
