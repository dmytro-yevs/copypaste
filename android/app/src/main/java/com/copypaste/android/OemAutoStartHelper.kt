package com.copypaste.android

import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.Settings

/**
 * Detects the device manufacturer and deep-links to the OEM-specific autostart
 * / protected-apps / battery-optimisation screen that controls background process
 * survival. Most OEMs do not expose a public API for this, so we use the
 * well-known (but undocumented) component intents that are derived from the
 * popular open-source AutoStarter library (github.com/judemanutd/AutoStarter)
 * and maintained community intel at dontkillmyapp.com.
 *
 * Design:
 *  1. Build an ordered list of candidate Intents for the detected manufacturer.
 *  2. For each candidate, check whether the component is resolvable (present on
 *     this device) using PackageManager.
 *  3. Launch the first resolvable Intent.
 *  4. If none resolve, fall back to the standard
 *     ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, then to the global battery
 *     settings screen.
 *
 * Because OEM ROMs vary significantly by model and MIUI/EMUI/ColorOS version,
 * no guarantees can be made that these intents will work on every device.
 * Always wrap launches in try/catch.
 *
 * The OEM data here is current as of 2025. Component paths change across ROM
 * versions — if a component is not found, the graceful fallback is used.
 */
object OemAutoStartHelper {

    /**
     * Returns a human-readable description of what the user should enable on
     * their device, e.g. "Autostart" for Xiaomi, "Protected apps" for Huawei.
     * Used by the UI to show context-specific instructions.
     */
    fun oemSettingsLabel(context: Context): String? = when (manufacturer()) {
        Manufacturer.XIAOMI -> "Autostart (Security > Permissions > Auto-start)"
        Manufacturer.HUAWEI -> "App launch (Settings > Battery > App launch)"
        Manufacturer.OPPO   -> "Auto-start (Security > Privacy Permissions > Startup)"
        Manufacturer.VIVO   -> "Auto-start (iQOO Security or Vivo PermissionManager)"
        Manufacturer.SAMSUNG -> "Sleeping apps (Device Care > Battery > Background usage)"
        Manufacturer.ONEPLUS -> "Auto-launch (Settings > Battery > Background app optimisation)"
        Manufacturer.ASUS   -> "Auto-start manager (Mobile Manager)"
        Manufacturer.LETV   -> "Auto-start (Letv Safe)"
        Manufacturer.NOKIA  -> "Power saver exceptions"
        Manufacturer.MEIZU  -> null   // no known component; falls through to battery settings
        Manufacturer.HTC    -> null
        Manufacturer.UNKNOWN -> null
    }

    /**
     * True if we have at least one candidate intent for this OEM (even if it may
     * not be resolvable at runtime on this exact device/ROM version).
     */
    fun hasOemScreen(context: Context): Boolean = manufacturer() != Manufacturer.UNKNOWN

    // ── Private helpers ─────────────────────────────────────────────────────

    /**
     * Manufacturer buckets we have a known autostart/protected-apps screen for.
     * Internal (not private) so the pure-logic [detectManufacturer] selector can
     * be unit-tested on the JVM without an Android runtime.
     */
    internal enum class Manufacturer {
        XIAOMI, HUAWEI, OPPO, VIVO, SAMSUNG, ONEPLUS, ASUS, LETV, NOKIA, MEIZU, HTC, UNKNOWN
    }

    private fun manufacturer(): Manufacturer = detectManufacturer(Build.MANUFACTURER)

    /**
     * Pure mapping from a raw `Build.MANUFACTURER` string to a [Manufacturer]
     * bucket. Extracted as a standalone function (no Android deps) so the OEM
     * selection logic is unit-testable without an instrumented device.
     *
     * Order matters: the OnePlus-on-Oppo-ROM special case must be evaluated
     * before the bare `oneplus` branch, and `oppo`/`realme` before it too, so a
     * genuine OnePlus device (OxygenOS) still resolves to [Manufacturer.ONEPLUS]
     * while OnePlus models shipping a ColorOS base resolve to [Manufacturer.OPPO].
     */
    internal fun detectManufacturer(rawManufacturer: String): Manufacturer {
        val mfr = rawManufacturer.lowercase()
        return when {
            mfr.contains("xiaomi") || mfr.contains("poco") || mfr.contains("redmi") -> Manufacturer.XIAOMI
            mfr.contains("huawei") || mfr.contains("honor") -> Manufacturer.HUAWEI
            // OnePlus models on a ColorOS/Oppo base, or genuine Oppo/Realme.
            mfr.contains("oppo") || mfr.contains("realme") ||
                    (mfr.contains("oneplus") && mfr.contains("oppo")) -> Manufacturer.OPPO
            mfr.contains("vivo") || mfr.contains("iqoo") -> Manufacturer.VIVO
            mfr.contains("samsung") -> Manufacturer.SAMSUNG
            mfr.contains("oneplus") -> Manufacturer.ONEPLUS
            mfr.contains("asus") -> Manufacturer.ASUS
            mfr.contains("letv") || mfr.contains("leeco") -> Manufacturer.LETV
            mfr.contains("nokia") -> Manufacturer.NOKIA
            mfr.contains("meizu") -> Manufacturer.MEIZU
            mfr.contains("htc") -> Manufacturer.HTC
            else -> Manufacturer.UNKNOWN
        }
    }

    /**
     * Returns the ordered list of candidate intents for the current manufacturer.
     * Each entry is tried in order; the first that is resolvable wins.
     * Components were compiled from judemanutd/AutoStarter and dontkillmyapp.com.
     */
    private fun oemIntents(context: Context): List<Intent> = when (manufacturer()) {

        Manufacturer.XIAOMI -> listOf(
            // MIUI 8 – 12
            componentIntent(
                "com.miui.securitycenter",
                "com.miui.permcenter.autostart.AutoStartManagementActivity"
            ),
            // MIUI 13+ (Xiaomi HyperOS)
            componentIntent(
                "com.miui.securitycenter",
                "com.miui.permcenter.autostart.AutoStartManagementActivity",
                extras = mapOf("package_name" to context.packageName)
            ),
        )

        Manufacturer.HUAWEI -> listOf(
            // EMUI 9+ (preferred — direct startup manager)
            componentIntent(
                "com.huawei.systemmanager",
                "com.huawei.systemmanager.startupmgr.ui.StartupNormalAppListActivity"
            ),
            // EMUI 5-8 — protected apps list
            componentIntent(
                "com.huawei.systemmanager",
                "com.huawei.systemmanager.optimize.process.ProtectActivity"
            ),
        )

        Manufacturer.OPPO -> listOf(
            // ColorOS 3–6
            componentIntent(
                "com.coloros.safecenter",
                "com.coloros.safecenter.permission.startup.StartupAppListActivity"
            ),
            componentIntent(
                "com.oppo.safe",
                "com.oppo.safe.permission.startup.StartupAppListActivity"
            ),
            componentIntent(
                "com.coloros.safecenter",
                "com.coloros.safecenter.startupapp.StartupAppListActivity"
            ),
        )

        Manufacturer.VIVO -> listOf(
            // FuntouchOS / OriginOS (iQOO variant)
            componentIntent(
                "com.iqoo.secure",
                "com.iqoo.secure.ui.phoneoptimize.AddWhiteListActivity"
            ),
            // FuntouchOS via PermissionManager
            componentIntent(
                "com.vivo.permissionmanager",
                "com.vivo.permissionmanager.activity.BgStartUpManagerActivity"
            ),
            // Older iQOO builds
            componentIntent(
                "com.iqoo.secure",
                "com.iqoo.secure.ui.phoneoptimize.BgStartUpManager"
            ),
        )

        Manufacturer.SAMSUNG -> listOf(
            // One UI 4+ (Device Care / Battery)
            componentIntent(
                "com.samsung.android.lool",
                "com.samsung.android.sm.battery.ui.usage.CheckableAppListActivity"
            ),
            // One UI 2-3
            componentIntent(
                "com.samsung.android.lool",
                "com.samsung.android.sm.ui.battery.BatteryActivity"
            ),
            componentIntent(
                "com.samsung.android.lool",
                "com.samsung.android.sm.battery.ui.BatteryActivity"
            ),
        )

        Manufacturer.ONEPLUS -> listOf(
            // OxygenOS
            componentIntent(
                "com.oneplus.security",
                "com.oneplus.security.chainlaunch.view.ChainLaunchAppListActivity"
            ),
            // OxygenOS / ColorOS merged builds — standard settings action
            Intent("com.android.settings.action.BACKGROUND_OPTIMIZE"),
        )

        Manufacturer.ASUS -> listOf(
            // ZenUI
            componentIntent(
                "com.asus.mobilemanager",
                "com.asus.mobilemanager.autostart.AutoStartActivity"
            ),
            componentIntent(
                "com.asus.mobilemanager",
                "com.asus.mobilemanager.powersaver.PowerSaverSettings"
            ),
        )

        Manufacturer.LETV -> listOf(
            componentIntent(
                "com.letv.android.letvsafe",
                "com.letv.android.letvsafe.AutobootManageActivity"
            ),
        )

        Manufacturer.NOKIA -> listOf(
            componentIntent(
                "com.evenwell.powersaving.g3",
                "com.evenwell.powersaving.g3.exception.PowerSaverExceptionActivity"
            ),
        )

        // Meizu / HTC: no confirmed resolvable component; fall through to battery settings.
        Manufacturer.MEIZU, Manufacturer.HTC, Manufacturer.UNKNOWN -> emptyList()
    }

    /** Build a component-based intent with optional String extras. */
    private fun componentIntent(
        pkg: String,
        cls: String,
        extras: Map<String, String> = emptyMap(),
    ): Intent = Intent().apply {
        component = ComponentName(pkg, cls)
        extras.forEach { (k, v) -> putExtra(k, v) }
    }

    /**
     * Ordered list of candidate OEM autostart/protected-apps intents for this
     * device's manufacturer, each tagged with [Intent.FLAG_ACTIVITY_NEW_TASK].
     * The caller should launch the first one that [isResolvable]; if none are,
     * fall back to the battery-exemption → app-details chain.
     *
     * Public so the onboarding flow can route the launch through its
     * ActivityResult launcher (instead of a bare startActivity) and so it can
     * apply a single in-flight gate across all permission/settings intents.
     */
    fun getOemIntentCandidates(context: Context): List<Intent> =
        oemIntents(context).map { it.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK) }

    /**
     * Ordered battery-exemption fallback chain (battery-opt exemption for this
     * package → global battery-opt list → app-details settings). Used when no
     * OEM-specific candidate is resolvable. Each intent carries
     * [Intent.FLAG_ACTIVITY_NEW_TASK].
     */
    fun getBatteryFallbackCandidates(context: Context): List<Intent> {
        val packageUri = Uri.parse("package:${context.packageName}")
        return listOf(
            Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS).apply { data = packageUri },
            Intent(Settings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS),
            Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS).apply { data = packageUri },
        ).map { it.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK) }
    }

    /**
     * Returns true if the intent can be resolved by at least one activity on this
     * device.
     *
     * IMPORTANT: we must NOT pass `PackageManager.MATCH_DEFAULT_ONLY` here. That
     * flag additionally requires the target activity to declare
     * `CATEGORY_DEFAULT` in an intent-filter. OEM autostart / protected-apps
     * activities (MIUI's AutoStartManagementActivity, EMUI's StartupNormalAppList
     * Activity, ColorOS StartupAppListActivity, etc.) are launched internally by
     * the ROM and almost never declare CATEGORY_DEFAULT — so resolving them with
     * MATCH_DEFAULT_ONLY returns null *even when the component is present and
     * exported*. That false-negative was the root cause of the "OEM settings
     * screen never opens" bug: every OEM candidate got filtered out before launch.
     *
     * For an explicit-component Intent, `resolveActivity` with flags `0` matches
     * purely by component existence + exported/visible state, which is exactly
     * what we want. For action-based fallback Intents (battery/app-details) flags
     * `0` is also correct — those system activities are always resolvable.
     */
    fun isResolvable(context: Context, intent: Intent): Boolean =
        context.packageManager.resolveActivity(intent, 0) != null
}
