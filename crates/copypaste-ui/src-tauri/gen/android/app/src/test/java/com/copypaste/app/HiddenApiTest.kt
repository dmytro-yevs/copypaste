package com.copypaste.app

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Test

/**
 * Stands in for `IClipboard`, whose methods gained `attributionTag` in API 30
 * and `deviceId` in 34. A device exposing both shapes is what the dispatch has
 * to be deterministic about.
 */
@Suppress("unused", "UNUSED_PARAMETER")
private class Overloaded {
    fun getPrimaryClip(callingPackage: String, userId: Int): String = "two"

    fun getPrimaryClip(
        callingPackage: String,
        attributionTag: String?,
        userId: Int,
        deviceId: Int,
    ): String = "four"

    fun hasPrimaryClip(callingPackage: String, userId: Int): Boolean = true
}

class HiddenApiTest {
    private val methods = Overloaded::class.java.methods

    /**
     * `Class.getMethods()` is documented as returning its elements in no
     * particular order, so taking the first name match makes the overload a
     * property of the run. The first assertion is that failure; the second is
     * that selection no longer depends on the order it was handed.
     */
    @Test
    fun theChosenOverloadDoesNotDependOnTheOrderTheMethodsArriveIn() {
        val forward = methods.sortedBy { it.toString() }.toTypedArray()
        val reversed = forward.reversedArray()

        assertNotEquals(
            forward.first { it.name == "getPrimaryClip" },
            reversed.first { it.name == "getPrimaryClip" },
        )
        assertEquals(
            HiddenApi.candidates(forward, "getPrimaryClip").first(),
            HiddenApi.candidates(reversed, "getPrimaryClip").first(),
        )
    }

    /** Fewest parameters is fewest values we have to guess. */
    @Test
    fun everyOverloadIsOfferedShortestFirst() {
        val candidates = HiddenApi.candidates(methods, "getPrimaryClip")

        assertEquals(listOf(2, 4), candidates.map { it.parameterTypes.size })
    }

    @Test
    fun aNameThatIsNotOnTheInterfaceHasNoCandidate() {
        assertEquals(emptyList<Any>(), HiddenApi.candidates(methods, "getPrimaryClipSource"))
    }

    /** AOSP's order: the first `String` is the caller, the first `int` the user. */
    @Test
    fun theFirstStringIsTheCallerAndTheFirstIntTheUser() {
        val four = HiddenApi.candidates(methods, "getPrimaryClip").last()

        val args = HiddenApi.arguments(four, emptyArray(), "com.android.shell", 11, 7)

        assertArrayEquals(arrayOf<Any?>("com.android.shell", null, 11, 7), args)
    }

    /** A listener stub is placed by its type, wherever the signature put it. */
    @Test
    fun aSuppliedArgumentTakesTheParameterItsTypeMatches() {
        val two = HiddenApi.candidates(methods, "hasPrimaryClip").single()

        val args = HiddenApi.arguments(two, arrayOf("com.example.caller"), "com.android.shell", 0, 0)

        assertArrayEquals(arrayOf<Any?>("com.example.caller", 0), args)
    }
}
