package com.copypaste.app

import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.KSerializer
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Test

class CaptureBridgeContractTest {
    @Serializable
    private data class Fixture(
        val probe: ProbeResult,
        val arms: List<ArmResult>,
        val armRequest: CaptureArmRequest,
        val notificationFacts: NotificationPermissionFacts,
        val tileFacts: TilePermissionFacts,
        val reads: List<ReadResult>,
        val drain: DrainResult,
        val empty: EmptyResult,
    )

    @Test
    fun checkedFixtureMatchesProductionSerializers() {
        val expected = checkNotNull(javaClass.getResource("/capture-bridge-contract.json"))
            .readText()
        val actual = CaptureBridgeJson.encode(Fixture.serializer(), fixture())

        assertEquals(
            Json.parseToJsonElement(expected),
            Json.parseToJsonElement(actual),
        )
    }

    @OptIn(ExperimentalSerializationApi::class)
    @Test
    fun productionSerializersKeepWireOptionality() {
        assertOptionality(ShizukuProbe.serializer())
        assertOptionality(ProbeResult.serializer())
        assertOptionality(ArmResult.serializer())
        assertOptionality(CaptureArmRequest.serializer())
        assertOptionality(NotificationPermissionFacts.serializer())
        assertOptionality(TileAddResultConstants.serializer())
        assertOptionality(TilePermissionFacts.serializer(), setOf("lastAddResult"))
        assertOptionality(
            ReadResult.serializer(),
            setOf("text", "sourceAppBundleId", "sourceAppName"),
        )
        assertOptionality(EmptyResult.serializer())
        assertOptionality(
            CapturedClip.serializer(),
            setOf("sourceAppBundleId", "sourceAppName"),
        )
        assertOptionality(DrainResult.serializer())
    }

    @OptIn(ExperimentalSerializationApi::class)
    private fun assertOptionality(
        serializer: KSerializer<*>,
        nullableFields: Set<String> = emptySet(),
    ) {
        val descriptor = serializer.descriptor
        val nullable = (0 until descriptor.elementsCount)
            .filter { descriptor.getElementDescriptor(it).isNullable }
            .mapTo(mutableSetOf()) { descriptor.getElementName(it) }
        val optional = (0 until descriptor.elementsCount)
            .filter(descriptor::isElementOptional)
            .mapTo(mutableSetOf(), descriptor::getElementName)

        assertEquals(nullableFields, nullable)
        assertEquals(emptySet<String>(), optional)
    }

    private fun fixture(): Fixture {
        val probe = ShizukuProbe(
            supported = true,
            installed = true,
            running = true,
            permission = true,
            enabled = true,
            toastSuppressed = false,
            rearmRequested = true,
        )
        return Fixture(
            ProbeResult(probe, enabled = true, listening = true),
            ReadOutcome.entries.mapIndexed { index, outcome ->
                ArmResult(
                    probe,
                    enabled = outcome == ReadOutcome.SUCCEEDED,
                    listening = outcome == ReadOutcome.SUCCEEDED,
                    outcome,
                    focused = true,
                    notificationPermission = index != ReadOutcome.entries.lastIndex,
                )
            },
            CaptureArmRequest("ongoing", "lost title", "lost body"),
            NotificationPermissionFacts(
                apiLevel = 36,
                granted = true,
                everAsked = true,
                showRationale = false,
            ),
            TilePermissionFacts(
                apiLevel = 36,
                lastAddResult = TileAddResultConstants.platform().added,
                resultConstants = TileAddResultConstants.platform(),
            ),
            listOf(
                ReadResult(
                    ReadOutcome.SUCCEEDED,
                    "captured",
                    1_700_000_000_001,
                    true,
                    "com.example.writer",
                    "Writer",
                ),
                ReadResult(ReadOutcome.EMPTY, null, 1_700_000_000_002, true, null, null),
                ReadResult(ReadOutcome.REFUSED, null, 1_700_000_000_003, true, null, null),
            ),
            DrainResult(
                CaptureSource.entries.mapIndexed { index, source ->
                    CapturedClip(
                        text = source.name.lowercase(),
                        source = source,
                        atMs = 1_700_000_000_100 + index,
                        sourceAppBundleId = if (source == CaptureSource.BACKGROUND) {
                            "com.example.writer"
                        } else {
                            null
                        },
                        sourceAppName = if (source == CaptureSource.BACKGROUND) "Writer" else null,
                    )
                },
                dropped = 2,
                probe,
            ),
            EmptyResult(),
        )
    }
}
