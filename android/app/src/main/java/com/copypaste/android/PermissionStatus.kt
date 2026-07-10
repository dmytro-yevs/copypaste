package com.copypaste.android

/**
 * Shared status for a runtime/special-access permission.
 *
 * Replaces the ad hoc `Boolean?` (null = not-applicable / indeterminate)
 * plumbing that grew independently in [OnboardingPermissions],
 * [PermissionsSettingsActivity] and [NotificationPermissionHelper]: those call
 * sites conflated "not granted" with "permanently denied" (Android 13+ caps
 * the POST_NOTIFICATIONS dialog after 2 denials, after which `launch()` is a
 * silent no-op — see [NotificationPermissionHelper.isPermanentlyDenied]).
 * Making the state explicit lets request logic branch on it directly instead
 * of re-deriving "permanently denied" ad hoc at every call site.
 */
enum class PermissionStatus {
    /** The permission is granted. */
    GRANTED,

    /** Not granted; the system may still show the rationale/request dialog. */
    DENIED,

    /**
     * Not granted, a previous request was made, and the OS now refuses to
     * show the rationale — a further `launch(permission)` would be a silent
     * no-op. The caller must route the user to system Settings instead.
     */
    PERMANENTLY_DENIED,

    /** The permission concept does not apply on this SDK level (implicitly satisfied). */
    NOT_APPLICABLE;

    /**
     * Mechanical bridge to the legacy `Boolean` "granted" rendering used by
     * the onboarding/permissions card composables: [GRANTED] and
     * [NOT_APPLICABLE] both render as "satisfied" (mirrors every existing
     * `if (sdkInt < X) true else isGranted` computation in this package).
     */
    fun isSatisfied(): Boolean = this == GRANTED || this == NOT_APPLICABLE
}

/**
 * Which action affordance a `PermissionCard` (OnboardingCards.kt) shows for a
 * given [PermissionStatus] (S10 Wave B / CopyPaste-myh8.10).
 */
enum class PermissionCardCta {
    /** Already satisfied — button renders as a disabled/ghost "granted" state. */
    SATISFIED,

    /** Not granted, the system may still show the request dialog. */
    REQUEST,

    /** Not granted and the OS suppresses the dialog — route the user to Settings. */
    OPEN_SETTINGS,
}

/**
 * Pure mapping from [PermissionStatus] to the CTA a `PermissionCard` shows.
 * NOT_APPLICABLE has no dedicated CTA visual (it is implicitly satisfied — see
 * [PermissionStatus.isSatisfied]) so it lands in [PermissionCardCta.SATISFIED],
 * the same bucket as GRANTED, mirroring every existing
 * `if (sdkInt < X) true else isGranted` computation in this package.
 */
fun permissionCardCta(status: PermissionStatus): PermissionCardCta = when (status) {
    PermissionStatus.GRANTED, PermissionStatus.NOT_APPLICABLE -> PermissionCardCta.SATISFIED
    PermissionStatus.DENIED -> PermissionCardCta.REQUEST
    PermissionStatus.PERMANENTLY_DENIED -> PermissionCardCta.OPEN_SETTINGS
}

/**
 * Pure Boolean -> [PermissionStatus] mapping for special-access grants that
 * have no rationale/permanent-denial concept (overlay `Settings.canDrawOverlays`,
 * battery `PowerManager.isIgnoringBatteryOptimizations`) — S10 Wave D/E
 * convention (CopyPaste-myh8.10): true -> GRANTED, false -> DENIED. Extracted
 * from the identical inline `if (x) GRANTED else DENIED` duplicated in
 * BackgroundCaptureSetupActivity and PermissionsSettingsActivity so the
 * mapping is unit-testable in one place.
 */
fun booleanGrantStatus(granted: Boolean): PermissionStatus =
    if (granted) PermissionStatus.GRANTED else PermissionStatus.DENIED
