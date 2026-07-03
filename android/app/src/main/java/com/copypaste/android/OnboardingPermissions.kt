package com.copypaste.android

import android.Manifest
import android.content.ActivityNotFoundException
import android.content.Intent
import android.os.Build
import android.provider.Settings
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.result.ActivityResultLauncher

/**
 * Pure "single request in flight" gate — models the state machine used by
 * [OnboardingPermissions.launchGated]: at most one permission/settings request
 * may be in flight at a time; further taps are ignored until the current one
 * completes (its ActivityResult callback releases the gate), so every
 * permission window can be opened in turn.
 *
 * Extracted verbatim from [OnboardingActivity]'s former `requestInFlight`
 * boolean field (CopyPaste-vp63.41) so the state transitions are unit
 * testable without an Activity/Compose harness. See
 * `OnboardingPermissionsTest` for the pure-logic tests.
 */
class RequestInFlightGate {
    private var inFlight = false

    /** True while a permission/settings request is in flight. */
    val isInFlight: Boolean get() = inFlight

    /**
     * Marks the gate as in-flight. Callers MUST check [isInFlight] first —
     * this call is unconditional, mirroring every call site in
     * [OnboardingPermissions] (check-then-act, not compare-and-swap).
     */
    fun acquire() {
        inFlight = true
    }

    /** Releases the gate so the next tap can proceed. */
    fun release() {
        inFlight = false
    }
}

/**
 * Non-UI permission-request controller for [OnboardingActivity].
 *
 * Owns the "one request in flight" gate ([RequestInFlightGate]) and the
 * fallback-intent-walking logic ([launchGated]) used by every permission /
 * settings launch on the onboarding screen. Moved verbatim out of
 * [OnboardingActivity] (CopyPaste-vp63.41) — no behavior change; the Activity
 * still owns the [ActivityResultLauncher]s (registration must happen at
 * Activity construction time) and forwards taps + launcher results here.
 */
class OnboardingPermissions(
    private val activity: ComponentActivity,
    private val notifLauncher: ActivityResultLauncher<String>,
    private val settingsLauncher: ActivityResultLauncher<Intent>,
    private val onOemToast: (String) -> Unit,
) {
    private val gate = RequestInFlightGate()

    /** Called from the Activity's launcher callbacks once a result comes back. */
    fun clearInFlight() {
        gate.release()
    }

    /**
     * Launch a Settings intent through [settingsLauncher] under the in-flight
     * gate, walking the supplied fallback [candidates] in order and using the
     * first that actually launches. If a tap arrives while another request is
     * pending it is ignored (the gate is held). Returns true if something was
     * launched; on failure of every candidate the gate is released so the user
     * can retry.
     */
    private fun launchGated(candidates: List<Intent>): Boolean {
        if (gate.isInFlight) {
            Log.d(TAG, "Ignoring tap: a permission/settings request is already in flight")
            return false
        }
        if (candidates.isEmpty()) return false
        gate.acquire()
        for (intent in candidates) {
            try {
                settingsLauncher.launch(intent)
                return true
            } catch (e: ActivityNotFoundException) {
                Log.w(TAG, "Settings intent not resolvable, trying next: ${e.message}")
            } catch (e: Exception) {
                Log.w(TAG, "Settings intent launch failed, trying next: ${e.message}")
            }
        }
        // Nothing launched — release the gate so the user isn't stuck.
        gate.release()
        Log.w(TAG, "No settings intent could be launched")
        return false
    }

    fun requestNotificationPermission() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return
        if (gate.isInFlight) {
            Log.d(TAG, "Ignoring tap: a permission/settings request is already in flight")
            return
        }
        // CopyPaste-l080: once POST_NOTIFICATIONS is permanently denied (Android 13+
        // caps the dialog after 2 denials) a launch() is a silent no-op. Route the
        // user to the app-notification-settings screen instead so the Grant button
        // is never dead.
        if (NotificationPermissionHelper.isPermanentlyDenied(activity)) {
            Log.i(TAG, "POST_NOTIFICATIONS permanently denied — opening app notification settings")
            launchGated(NotificationPermissionHelper.appNotificationSettingsIntents(activity))
            return
        }
        gate.acquire()
        NotificationPermissionHelper.markRequested(activity)
        notifLauncher.launch(Manifest.permission.POST_NOTIFICATIONS)
    }

    fun requestOverlayPermission() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.M) return
        launchGated(
            listOf(
                Intent(
                    Settings.ACTION_MANAGE_OVERLAY_PERMISSION,
                    android.net.Uri.fromParts("package", activity.packageName, null)
                ).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            )
        )
    }

    fun requestBatteryOptimizationExemption() {
        // Battery-exemption intent first, then the global battery-opt list as
        // a fallback for OEMs that don't expose the per-package action.
        launchGated(OemAutoStartHelper.getBatteryFallbackCandidates(activity))
    }

    /**
     * Open the OEM-specific autostart / protected-apps settings screen, routed
     * through [settingsLauncher] (so the return triggers a refresh) and under
     * the shared in-flight gate. Tries each resolvable OEM candidate first, then
     * the battery-exemption → app-details fallback chain. Every launch is
     * guarded so an unresolvable OEM intent can never crash the app.
     */
    fun openOemAutoStart() {
        // Try resolvable OEM-specific candidates first, then ALL OEM candidates
        // (in case resolveActivity under-reports a hidden-but-launchable
        // component), then the generic battery → app-details → settings chain.
        // launchGated walks the list and uses the first that actually launches,
        // catching ActivityNotFoundException per-candidate so a missing OEM
        // component can never crash or dead-end the flow.
        val resolvable = OemAutoStartHelper.getOemIntentCandidates(activity)
            .filter { OemAutoStartHelper.isResolvable(activity, it) }
        val allOem = OemAutoStartHelper.getOemIntentCandidates(activity)
        val fallback = OemAutoStartHelper.getBatteryFallbackCandidates(activity)
        val launched = launchGated(resolvable + allOem + fallback)
        if (launched) {
            val label = OemAutoStartHelper.oemSettingsLabel(activity)
            val hint = if (label != null) {
                activity.getString(R.string.oem_autostart_toast_labeled, label)
            } else {
                activity.getString(R.string.oem_autostart_toast_generic)
            }
            onOemToast(hint)
        }
    }

    private companion object {
        private const val TAG = "OnboardingActivity"
    }
}
