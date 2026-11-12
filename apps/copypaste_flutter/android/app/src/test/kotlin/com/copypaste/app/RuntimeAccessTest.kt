package com.copypaste.app

import java.util.concurrent.Executor
import org.json.JSONObject
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class RuntimeAccessTest {
    @Test fun sendsTheMethodEnvelopeAndUnwrapsTheResponseVariant() {
        var sent: String? = null
        val access = JsonRuntimeAccess { method ->
            sent = method
            """{"id":0,"ok":true,"data":{"item_page":{"items":[],"next_cursor":null}}}"""
        }

        val reply = access.request("history_query", JSONObject().put("limit", 30))

        val envelope = JSONObject(sent!!)
        assertEquals("history_query", envelope.getString("method"))
        assertEquals(30, envelope.getJSONObject("params").getInt("limit"))
        reply as RuntimeReply.Ok
        assertEquals("item_page", reply.kind)
        assertEquals(0, reply.objectData()!!.getJSONArray("items").length())
    }

    @Test fun omitsParamsForUnitMethods() {
        var sent: String? = null
        JsonRuntimeAccess { method ->
            sent = method
            """{"id":0,"ok":true,"data":{"empty":{}}}"""
        }.request("status")

        assertFalse(JSONObject(sent!!).has("params"))
    }

    @Test fun mapsKnownUnknownAndMalformedFailures() {
        fun failure(raw: String) = JsonRuntimeAccess { raw }.request("status") as RuntimeReply.Failure

        val unsupported = failure(
            """{"id":0,"ok":false,"error":"This action needs the CopyPaste app.","error_code":"unsupported"}""",
        )
        assertEquals(RuntimeErrorCode.UNSUPPORTED, unsupported.code)
        assertEquals("This action needs the CopyPaste app.", unsupported.message)
        assertEquals(
            RuntimeErrorCode.UNKNOWN,
            failure("""{"id":0,"ok":false,"error":"Later.","error_code":"future_code"}""").code,
        )
        assertEquals(RuntimeErrorCode.INTERNAL, failure("not json").code)
        assertEquals(RuntimeErrorCode.INTERNAL, failure("""{"id":0,"ok":false}""").code)
        assertEquals(RuntimeErrorCode.INTERNAL, failure("""{"id":0,"ok":true,"data":{"a":1,"b":2}}""").code)
    }

    @Test fun transportFailureBecomesAnInternalReply() {
        val reply = JsonRuntimeAccess { throw UnsatisfiedLinkError("no bridge") }.request("status")

        assertEquals(RuntimeErrorCode.INTERNAL, (reply as RuntimeReply.Failure).code)
    }

    @Test fun workerRunsOffTheCallerAndDeliversOnTheDeliveryExecutor() {
        val work = mutableListOf<Runnable>()
        val delivered = mutableListOf<Runnable>()
        val replies = mutableListOf<RuntimeReply>()
        val worker = RuntimeAccessWorker(
            JsonRuntimeAccess { """{"id":0,"ok":true,"data":{"empty":{}}}""" },
            Executor { work += it },
            Executor { delivered += it },
        )

        worker.request("status") { replies += it }
        assertTrue(replies.isEmpty())
        work.single().run()
        assertTrue(replies.isEmpty())
        delivered.single().run()

        assertEquals("empty", (replies.single() as RuntimeReply.Ok).kind)
    }
}
