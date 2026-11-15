package com.copypaste.app

import android.app.Activity
import android.app.Application
import android.content.SharedPreferences
import android.os.Bundle
import android.view.Window
import android.view.WindowManager
import java.util.WeakHashMap

/** One persisted device policy applies to all Flutter and native activities. */
internal object ScreenshotProtection : Application.ActivityLifecycleCallbacks {
    private const val preferenceKey = "blockScreenshots"
    private var preferences: SharedPreferences? = null
    private val activities = WeakHashMap<Activity, Unit>()
    var blocked: Boolean = false
        private set

    fun install(application: Application) {
        if (preferences != null) return
        preferences = application.getSharedPreferences("copypaste.security", Application.MODE_PRIVATE)
        blocked = preferences!!.getBoolean(preferenceKey, false)
        application.registerActivityLifecycleCallbacks(this)
    }

    fun setBlocked(value: Boolean): Boolean {
        val before = blocked
        blocked = value
        applyToActivities()
        if (preferences?.edit()?.putBoolean(preferenceKey, value)?.commit() == true) return true
        blocked = before
        applyToActivities()
        return false
    }

    fun apply(window: Window) {
        if (blocked) window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
        else window.clearFlags(WindowManager.LayoutParams.FLAG_SECURE)
    }

    private fun applyToActivities() {
        for (activity in activities.keys.toList()) {
            if (!activity.isDestroyed) apply(activity.window)
        }
    }

    override fun onActivityCreated(activity: Activity, savedInstanceState: Bundle?) {
        activities[activity] = Unit
        apply(activity.window)
    }

    override fun onActivityResumed(activity: Activity) = apply(activity.window)
    override fun onActivityDestroyed(activity: Activity) { activities.remove(activity) }
    override fun onActivityStarted(activity: Activity) = Unit
    override fun onActivityPaused(activity: Activity) = Unit
    override fun onActivityStopped(activity: Activity) = Unit
    override fun onActivitySaveInstanceState(activity: Activity, state: Bundle) = Unit
}
