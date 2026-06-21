package com.copypaste.android

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
