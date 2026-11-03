package com.copypaste.app

import java.io.File
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/** Checks the fixture shared with Flutter's `native_text_vectors_test.dart`. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class NativeTextVectorsTest {
    private val vectors = JSONObject(File("../../test/fixtures/native_text_vectors.json").readText())

    @Test fun fileNamesMatchSharedVectors() {
        val cases = vectors.getJSONArray("safeFileName")
        for (index in 0 until cases.length()) {
            val case = cases.getJSONObject(index)
            assertEquals(
                case.getString("input"),
                case.getString("expected"),
                safeFileName(case.getString("input"), case.getInt("maxBytes")),
            )
        }
    }

    @Test fun capturePreviewsMatchSharedVectors() {
        val cases = vectors.getJSONArray("captureNotificationPreview")
        for (index in 0 until cases.length()) {
            val case = cases.getJSONObject(index)
            assertEquals(case.getString("expected"), AndroidCaptureFeedback.textPreview(case.getString("input")))
        }
    }
}
