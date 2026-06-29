package com.copypaste.android

/**
 * CopyPaste-bdac.42: which transports to probe during a "Test connection" run.
 *
 * A backend is included iff its toggle is ENABLED AND its required config fields
 * are non-blank. This mirrors the additive fan-out model (CopyPaste-26zi): relay
 * and Supabase are independent; either, both, or neither may be selected.
 *
 * [relay]    — true when relayEnabled AND relayUrl is non-blank.
 * [supabase] — true when supabaseEnabled AND supabaseUrl + supabaseAnonKey are non-blank.
 */
data class BackendsToTest(val relay: Boolean, val supabase: Boolean)

/**
 * Select which transports the "Test connection" button should probe.
 *
 * Pure function — no I/O, no coroutines. Accepts draft (unsaved) field values so
 * the user can test before tapping Save.
 */
fun selectTestBackends(
    relayEnabled: Boolean,
    relayUrl: String,
    supabaseEnabled: Boolean,
    supabaseUrl: String,
    supabaseAnonKey: String,
): BackendsToTest = BackendsToTest(
    relay = relayEnabled && relayUrl.isNotBlank(),
    supabase = supabaseEnabled && supabaseUrl.isNotBlank() && supabaseAnonKey.isNotBlank(),
)

/**
 * Return the value in [steps] whose absolute distance to [raw] is smallest.
 * Used to snap an existing config value to the nearest stepped-slider position
 * on load, so arbitrary legacy values always display cleanly.
 */
internal fun snapToNearestLong(steps: LongArray, raw: Long): Long {
    var best = steps[0]
    var bestDist = kotlin.math.abs(raw - best)
    for (i in 1 until steps.size) {
        val d = kotlin.math.abs(raw - steps[i])
        if (d < bestDist) {
            bestDist = d
            best = steps[i]
        }
    }
    return best
}
