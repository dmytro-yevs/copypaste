package com.copypaste.app

import java.util.Locale

internal enum class ExternalReadDecision {
    READ,
    SKIP_EXCLUDED,
    SKIP_UNKNOWN,
}

/** The pre-read half of the app-exclusion contract. */
internal object CaptureExclusions {
    private var configured = false
    private var bundleIds = emptySet<String>()

    @Synchronized
    fun replace(configured: Boolean, values: List<String>) {
        this.configured = configured
        bundleIds = values
            .asSequence()
            .map(String::trim)
            .filter(String::isNotEmpty)
            .map { it.lowercase(Locale.ROOT) }
            .toSet()
    }

    @Synchronized
    fun decide(sourcePackage: String?): ExternalReadDecision {
        if (!configured) return ExternalReadDecision.SKIP_UNKNOWN
        if (bundleIds.isEmpty()) return ExternalReadDecision.READ
        val source = sourcePackage
            ?.trim()
            ?.takeIf(String::isNotEmpty)
            ?.lowercase(Locale.ROOT)
            ?: return ExternalReadDecision.SKIP_UNKNOWN
        return if (source in bundleIds) {
            ExternalReadDecision.SKIP_EXCLUDED
        } else {
            ExternalReadDecision.READ
        }
    }
}
