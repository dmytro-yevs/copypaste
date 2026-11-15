package com.copypaste.app

import android.os.Bundle
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel
import java.util.UUID
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors

class PairingPresentationActivity : FlutterActivity() {
    companion object {
        const val contextIdExtra = "pairing_context_id"
        private const val protectedRoutePrefix = "/protected-pairing/"
        private const val protectedContextChannel = "com.copypaste.app/pairing_presentation_context"
        private val ceremonies = ConcurrentHashMap<String, String>()
        private val generations = ConcurrentHashMap<String, Long>()
        private val activeActivities = ConcurrentHashMap<String, PairingPresentationActivity>()

        fun register(ceremonyId: String): String = UUID.randomUUID().toString().also {
            ceremonies[it] = ceremonyId
        }

        fun unregister(contextId: String) {
            ceremonies.remove(contextId)
            generations.remove(contextId)
        }

        fun setGeneration(contextId: String, generation: Long) {
            generations[contextId] = generation
        }

        fun activeActivity(contextId: String?): PairingPresentationActivity? =
            contextId?.let(activeActivities::get)

        fun close(contextId: String?): Boolean {
            val activity = activeActivity(contextId) ?: return false
            activity.requestClose(null)
            return true
        }

        private fun context(contextId: String): PairingContext? {
            val ceremonyId = ceremonies[contextId] ?: return null
            val generation = generations[contextId] ?: return null
            return PairingContext(contextId, ceremonyId, generation)
        }
    }

    private val nativeExecutor: ExecutorService = Executors.newSingleThreadExecutor()
    private val contextId: String? get() = intent.getStringExtra(contextIdExtra)

    override fun onCreate(savedInstanceState: Bundle?) {
        ScreenshotProtection.install(application)
        ScreenshotProtection.apply(window)
        super.onCreate(savedInstanceState)
        contextId?.let { activeActivities[it] = this }
    }

    override fun getInitialRoute(): String? = contextId?.let { "$protectedRoutePrefix$it" }

    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, protectedContextChannel)
            .setMethodCallHandler { call, result ->
                when (call.method) {
                    "closeContext" -> requestClose(result)
                    else -> nativeExecutor.execute { handleNativeCall(call, result) }
                }
            }
    }

    override fun onDestroy() {
        contextId?.let { id ->
            activeActivities.remove(id)
            unregister(id)
            nativeExecutor.execute {
                if (NativeProtectedPairing.active(id)) NativeProtectedPairing.cancel(id)
                NativeProtectedPairing.detach(id)
            }
        }
        nativeExecutor.shutdown()
        super.onDestroy()
    }

    private fun handleNativeCall(call: MethodCall, result: MethodChannel.Result) {
        val context = activeContext()
        if (context == null) {
            reply(result) { result.error("context_inactive", null, null) }
            return
        }
        when (call.method) {
            "isContextActive" -> reply(result) { result.success(true) }
            "ceremonyId" -> reply(result) { result.success(context.ceremonyId) }
            "status" -> {
                val status = NativeProtectedPairing.status(context.id, context.generation)
                reply(result) {
                    if (status == null) result.error("context_inactive", null, null)
                    else result.success(mapOf("state" to status[0], "expiresInMs" to status[1]))
                }
            }
            "revealQr" -> reply(result) {
                result.success(NativeProtectedPairing.revealQr(context.ceremonyId, context.id, context.generation))
            }
            "revealSas" -> reply(result) {
                result.success(NativeProtectedPairing.revealSas(context.id, context.generation))
            }
            "joinManual" -> {
                val code = call.argument<String>("code")
                val address = call.argument<String>("address")
                reply(result) {
                    if (code.isNullOrBlank() || address.isNullOrBlank()) result.error("invalid_arguments", null, null)
                    else result.success(NativeProtectedPairing.join(context.id, context.generation, code, address))
                }
            }
            "joinQr" -> {
                val uri = call.argument<String>("uri")
                reply(result) {
                    if (uri.isNullOrBlank()) result.error("invalid_arguments", null, null)
                    else result.success(NativeProtectedPairing.joinUri(context.id, context.generation, uri))
                }
            }
            "confirm" -> {
                val sas = call.argument<String>("sas")
                val accept = call.argument<Boolean>("accept")
                reply(result) {
                    if (sas.isNullOrBlank() || accept == null) result.error("invalid_arguments", null, null)
                    else result.success(NativeProtectedPairing.decide(context.id, context.generation, sas, accept))
                }
            }
            "cancel" -> reply(result) { result.success(NativeProtectedPairing.cancel(context.id)) }
            else -> reply(result) { result.notImplemented() }
        }
    }

    private fun activeContext(): PairingContext? {
        val id = contextId ?: return null
        val context = context(id) ?: return null
        return context.takeIf { NativeProtectedPairing.active(it.id) }
    }

    private fun requestClose(result: MethodChannel.Result?) {
        val id = contextId
        if (id == null) {
            result?.success(false)
            return
        }
        nativeExecutor.execute {
            val active = NativeProtectedPairing.active(id)
            val canClose = !active || NativeProtectedPairing.cancel(id)
            if (!active) NativeProtectedPairing.detach(id)
            reply(result) {
                if (!canClose) result?.error("decision_busy", null, null)
                else {
                    result?.success(true)
                    finish()
                }
            }
        }
    }

    private fun reply(result: MethodChannel.Result?, callback: () -> Unit) {
        runOnUiThread(callback)
    }

    private data class PairingContext(
        val id: String,
        val ceremonyId: String,
        val generation: Long,
    )
}
