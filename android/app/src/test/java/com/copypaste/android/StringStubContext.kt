package com.copypaste.android

import android.content.Context
import android.content.ContextWrapper
import android.content.res.Resources

/**
 * Test-only [ContextWrapper] that stubs `getString(...)` and delegates
 * everything else (NotificationManager, SharedPreferences, etc.) to the real
 * Robolectric context.
 *
 * This module has `isIncludeAndroidResources` unset for JVM unit tests, so
 * Robolectric has no merged resource table — any real
 * `context.getString(R.string...)` call throws `Resources$NotFoundException`
 * (see [RepairedSettingsConsumersTest] / [NotificationsTabSettingsTest] kdocs
 * for the same, pre-existing constraint). Production code that legitimately
 * needs to call `getString` (e.g. [NotificationHelper.notifyNativeUnavailable]
 * as a security-sentinel notification, or [ServiceNotifications]) still needs
 * to be exercised by tests that reach it incidentally — wrapping the context
 * passed into that code with this class lets the real logic run unmodified
 * while only the resource-text lookup is faked, instead of the whole call
 * throwing an unrelated `Resources$NotFoundException`.
 */
internal class StringStubContext(base: Context) : ContextWrapper(base) {
    override fun getString(resId: Int): String = "stub_$resId"
    override fun getString(resId: Int, vararg formatArgs: Any?): String = "stub_$resId"

    // S13 Wave c: notif_content_today became a <plurals> resource, so
    // ServiceNotifications now calls context.resources.getQuantityString(...)
    // instead of context.getString(...) — needs the same no-merged-resource-table
    // stub as getString above, or it throws Resources$NotFoundException.
    private val stubResources: Resources by lazy { StubResources(super.getResources()) }
    override fun getResources(): Resources = stubResources

    // ContextWrapper.getApplicationContext() delegates to the base context's
    // REAL (unwrapped) Application by default, which would bypass the
    // getString stub above for any caller that stores `context.applicationContext`
    // (e.g. ClipboardRepository.appContext) rather than the context it was
    // constructed with. Returning `this` keeps the stub in effect through that
    // indirection too.
    override fun getApplicationContext(): Context = this
}

// android.content.res.Resources has no public delegating wrapper, so this
// subclass reuses the base Resources' AssetManager/DisplayMetrics/Configuration
// (the deprecated Resources(AssetManager, DisplayMetrics, Configuration)
// constructor) — every other lookup behaves exactly like [base], only
// getQuantityString is stubbed.
@Suppress("DEPRECATION")
private class StubResources(base: Resources) :
    Resources(base.assets, base.displayMetrics, base.configuration) {
    override fun getQuantityString(id: Int, quantity: Int): String = "stub_$id"
    override fun getQuantityString(id: Int, quantity: Int, vararg formatArgs: Any?): String = "stub_$id"
}
