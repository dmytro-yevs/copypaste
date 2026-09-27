package com.copypaste.app

import android.content.Context
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.net.wifi.WifiManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.Log
import java.util.concurrent.ConcurrentHashMap

/**
 * Android-owned LAN discovery: a multicast lock plus [NsdManager].
 *
 * Raw mDNS sockets (mdns-sd) are dropped on many Android stacks unless the
 * process holds a Wi-Fi multicast lock. [NsdManager] is the platform DNS-SD
 * path and does not depend on that lock remaining held after an activity dies.
 */
internal class LanDiscovery(
    context: Context,
    private val onChange: () -> Unit = {},
) {
    private val app = context.applicationContext
    private val nsd = app.getSystemService(Context.NSD_SERVICE) as? NsdManager
    private val handler = Handler(Looper.getMainLooper())
    private var multicastLock: WifiManager.MulticastLock? = null
    private val browse = BrowseLifecycle()
    private val pendingBrowse = mutableListOf<(Boolean) -> Unit>()
    private val browseTimeout = Runnable {
        Log.w(TAG, "NSD browse start timed out")
        finishBrowse(false)
    }
    private var registered: NsdServiceInfo? = null
    private var advertising: Advertisement? = null
    private var registrationInFlight = false
    private val resolved = ConcurrentHashMap<String, ResolvedPeer>()
    private val resolveQueue = ArrayDeque<NsdServiceInfo>()
    private var resolving = false

    private val discoveryListener = object : NsdManager.DiscoveryListener {
        override fun onDiscoveryStarted(regType: String) {
            finishBrowse(true)
        }

        override fun onDiscoveryStopped(serviceType: String) {
            stopBrowseState()
        }

        override fun onStartDiscoveryFailed(serviceType: String, errorCode: Int) {
            Log.w(TAG, "NSD browse failed to start: $errorCode")
            finishBrowse(false)
        }

        override fun onStopDiscoveryFailed(serviceType: String, errorCode: Int) {
            Log.w(TAG, "NSD browse failed to stop")
        }

        override fun onServiceFound(service: NsdServiceInfo) {
            if (service.serviceType?.contains(SERVICE_TYPE_TOKEN) != true) return
            if (registered?.serviceName == service.serviceName || advertising?.name == service.serviceName) return
            enqueueResolve(service)
        }

        override fun onServiceLost(service: NsdServiceInfo) {
            resolved.remove(service.serviceName)
            onChange()
        }
    }

    private val registrationListener = object : NsdManager.RegistrationListener {
        override fun onServiceRegistered(serviceInfo: NsdServiceInfo) {
            synchronized(this@LanDiscovery) {
                registered = serviceInfo
                registrationInFlight = false
            }
        }

        override fun onRegistrationFailed(serviceInfo: NsdServiceInfo, errorCode: Int) {
            synchronized(this@LanDiscovery) {
                registered = null
                advertising = null
                registrationInFlight = false
            }
            Log.w(TAG, "NSD registration failed: $errorCode")
        }

        override fun onServiceUnregistered(serviceInfo: NsdServiceInfo) {
            synchronized(this@LanDiscovery) {
                if (registered?.serviceName == serviceInfo.serviceName) registered = null
                advertising = null
                registrationInFlight = false
            }
        }

        override fun onUnregistrationFailed(serviceInfo: NsdServiceInfo, errorCode: Int) {
            Log.w(TAG, "NSD unregistration failed")
        }
    }

    fun acquireMulticastLock(): Boolean = try {
        val wifi = app.getSystemService(Context.WIFI_SERVICE) as? WifiManager
        val lock = multicastLock ?: wifi?.createMulticastLock(TAG)?.also {
            it.setReferenceCounted(false)
            multicastLock = it
        }
        if (lock != null && !lock.isHeld) lock.acquire()
        lock?.isHeld == true
    } catch (e: Throwable) {
        Log.w(TAG, "the multicast lock could not be acquired", e)
        false
    }

    fun startBrowse(onReady: (Boolean) -> Unit) {
        val shouldStart = synchronized(this) {
            when (browse.begin()) {
                BrowseStart.READY -> false
                BrowseStart.WAITING -> {
                    pendingBrowse += onReady
                    return
                }
                BrowseStart.START -> {
                    pendingBrowse += onReady
                    true
                }
            }
        }
        if (!shouldStart) {
            onReady(true)
            return
        }
        val manager = nsd
        if (manager == null) {
            finishBrowse(false)
            return
        }
        try {
            manager.discoverServices(SERVICE_TYPE, NsdManager.PROTOCOL_DNS_SD, discoveryListener)
            handler.postDelayed(browseTimeout, BROWSE_START_TIMEOUT_MS)
        } catch (e: Throwable) {
            Log.w(TAG, "NSD browse could not start", e)
            finishBrowse(false)
        }
    }

    @Synchronized
    fun advertise(name: String, port: Int, attributes: Map<String, String>): Boolean {
        val manager = nsd ?: return false
        if (name.isBlank() || port !in 1..65535) return false
        val requested = Advertisement(name.take(MAX_SERVICE_NAME), port, attributes)
        if (advertising == requested && (registrationInFlight || registered != null)) return true
        if (registrationInFlight) return false
        stopAdvertise()
        val info = NsdServiceInfo().apply {
            serviceName = requested.name
            serviceType = SERVICE_TYPE
            setPort(requested.port)
            requested.attributes.forEach { (key, value) ->
                if (key.isNotEmpty() && value.isNotEmpty()) {
                    setAttribute(key.take(MAX_TXT_KEY), value.take(MAX_TXT_VALUE))
                }
            }
        }
        return try {
            advertising = requested
            registrationInFlight = true
            manager.registerService(info, NsdManager.PROTOCOL_DNS_SD, registrationListener)
            true
        } catch (e: Throwable) {
            advertising = null
            registrationInFlight = false
            Log.w(TAG, "NSD registration could not start", e)
            false
        }
    }

    fun peers(): List<ResolvedPeer> = resolved.values.toList()

    /**
     * The lock and NSD session belong to the process, not the activity. An
     * activity destroy that released them made discovery look permanently
     * unavailable after the first configuration change or backgrounding.
     */
    fun releaseProcess() {
        stopBrowse()
        stopAdvertise()
        resolved.clear()
        try {
            multicastLock?.takeIf { it.isHeld }?.release()
        } catch (e: Throwable) {
            Log.w(TAG, "the multicast lock could not be released", e)
        }
    }

    private fun stopBrowse() {
        val wasRunning = synchronized(this) {
            handler.removeCallbacks(browseTimeout)
            browse.stop()
        }
        if (!wasRunning) return
        notifyBrowseStopped()
        try {
            nsd?.stopServiceDiscovery(discoveryListener)
        } catch (e: Throwable) {
            Log.w(TAG, "NSD browse could not stop", e)
        }
    }

    private fun stopAdvertise() {
        val current = registered ?: return
        try {
            nsd?.unregisterService(registrationListener)
        } catch (e: Throwable) {
            Log.w(TAG, "NSD registration could not stop", e)
        }
        registered = null
        advertising = null
        registrationInFlight = false
    }

    private fun finishBrowse(available: Boolean) {
        val callbacks = synchronized(this) {
            if (!browse.finish(available)) return
            handler.removeCallbacks(browseTimeout)
            pendingBrowse.toList().also { pendingBrowse.clear() }
        }
        callbacks.forEach { callback -> callback(available) }
    }

    private fun stopBrowseState() {
        val wasRunning = synchronized(this) {
            handler.removeCallbacks(browseTimeout)
            browse.stop()
        }
        if (wasRunning) notifyBrowseStopped()
    }

    private fun notifyBrowseStopped() {
        val callbacks = synchronized(this) {
            pendingBrowse.toList().also { pendingBrowse.clear() }
        }
        callbacks.forEach { callback -> callback(false) }
    }

    @Synchronized
    private fun enqueueResolve(service: NsdServiceInfo) {
        if (resolveQueue.any { it.serviceName == service.serviceName }) return
        resolveQueue.addLast(service)
        pumpResolve()
    }

    @Synchronized
    private fun pumpResolve() {
        if (resolving) return
        val next = resolveQueue.removeFirstOrNull() ?: return
        resolving = true
        val manager = nsd
        if (manager == null) {
            resolving = false
            return
        }
        try {
            manager.resolveService(
                next,
                object : NsdManager.ResolveListener {
                    override fun onResolveFailed(serviceInfo: NsdServiceInfo, errorCode: Int) {
                        finishResolve()
                    }

                    override fun onServiceResolved(serviceInfo: NsdServiceInfo) {
                        hostOf(serviceInfo)?.let { host ->
                            if (serviceInfo.port > 0) {
                                resolved[serviceInfo.serviceName] = ResolvedPeer(
                                    serviceName = serviceInfo.serviceName,
                                    host = host,
                                    port = serviceInfo.port,
                                    attributes = attributesOf(serviceInfo),
                                )
                                onChange()
                            }
                        }
                        finishResolve()
                    }
                },
            )
        } catch (e: Throwable) {
            Log.w(TAG, "NSD resolve could not start", e)
            finishResolve()
        }
    }

    @Synchronized
    private fun finishResolve() {
        resolving = false
        pumpResolve()
    }

    private fun hostOf(info: NsdServiceInfo): String? {
        if (Build.VERSION.SDK_INT >= 34) {
            val addresses = info.hostAddresses
            if (!addresses.isNullOrEmpty()) {
                return addresses.firstOrNull()?.hostAddress
            }
        }
        @Suppress("DEPRECATION")
        return info.host?.hostAddress
    }

    private fun attributesOf(info: NsdServiceInfo): Map<String, String> {
        val raw = info.attributes ?: return emptyMap()
        val out = LinkedHashMap<String, String>()
        raw.forEach { (key, bytes) ->
            if (key.isNullOrEmpty() || bytes == null) return@forEach
            out[key] = bytes.toString(Charsets.UTF_8)
        }
        return out
    }

    data class ResolvedPeer(
        val serviceName: String,
        val host: String,
        val port: Int,
        val attributes: Map<String, String>,
    )

    private data class Advertisement(
        val name: String,
        val port: Int,
        val attributes: Map<String, String>,
    )

    companion object {
        const val TAG = "copypaste-mdns"
        const val SERVICE_TYPE = "_copypaste._tcp."
        private const val SERVICE_TYPE_TOKEN = "_copypaste._tcp"
        private const val MAX_SERVICE_NAME = 63
        private const val MAX_TXT_KEY = 9
        private const val MAX_TXT_VALUE = 200
        private const val BROWSE_START_TIMEOUT_MS = 5_000L
    }
}

internal enum class BrowseStart { START, WAITING, READY }

internal class BrowseLifecycle {
    private var state = State.STOPPED

    fun begin(): BrowseStart = when (state) {
        State.STOPPED -> {
            state = State.STARTING
            BrowseStart.START
        }
        State.STARTING -> BrowseStart.WAITING
        State.STARTED -> BrowseStart.READY
    }

    fun finish(available: Boolean): Boolean {
        if (state != State.STARTING) return false
        state = if (available) State.STARTED else State.STOPPED
        return true
    }

    fun stop(): Boolean {
        if (state == State.STOPPED) return false
        state = State.STOPPED
        return true
    }

    private enum class State { STOPPED, STARTING, STARTED }
}
