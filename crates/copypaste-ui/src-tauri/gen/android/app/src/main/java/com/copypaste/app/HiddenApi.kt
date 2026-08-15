package com.copypaste.app

import java.lang.reflect.Method

/**
 * Choosing and calling one method on a hidden AIDL interface whose exact
 * signature depends on the API level.
 *
 * `IClipboard`'s methods have gained parameters repeatedly — `attributionTag`
 * in 30, `deviceId` in 34 — so the argument vector is built from the method's
 * own parameter types rather than from one shipped AIDL.
 */
object HiddenApi {
    /**
     * Every candidate for `name`, most specific last.
     *
     * `Class.getMethods()` documents its result as "not sorted and ... in no
     * particular order", so taking the first name match makes the chosen
     * overload a property of the run rather than of the device. Bridge and
     * synthetic methods are the usual second candidate and are dropped; what
     * survives is ordered by arity and then by parameter type name, so two
     * devices exposing the same class agree on the answer.
     *
     * The caller takes the first: fewest parameters is fewest values we have to
     * guess, and every parameter beyond the AOSP ones is guessed.
     */
    fun candidates(methods: Array<Method>, name: String): List<Method> =
        methods
            .filter { it.name == name && !it.isSynthetic && !it.isBridge }
            .sortedWith(
                compareBy(
                    { it.parameterTypes.size },
                    { it.parameterTypes.joinToString(",") { type -> type.name } },
                ),
            )

    /**
     * Fill each parameter from its declared type.
     *
     * The order AOSP has used throughout is `(…, String callingPackage,
     * [String attributionTag], int userId, [int deviceId])`, so the first
     * `String` is the calling package and the first `int` the user id. Anything
     * else is passed as a type-appropriate zero, which is what every one of
     * these parameters defaults to.
     */
    fun arguments(
        method: Method,
        specific: Array<out Any>,
        callingPackage: String,
        userId: Int,
        deviceId: Int,
    ): Array<Any?> {
        var sawString = false
        var sawInt = false
        return method.parameterTypes
            .map { type ->
                val given = specific.firstOrNull { type.isAssignableFrom(it.javaClass) }
                when {
                    given != null -> given
                    type == String::class.java && !sawString -> {
                        sawString = true
                        callingPackage
                    }
                    type == String::class.java -> null
                    type == Int::class.javaPrimitiveType && !sawInt -> {
                        sawInt = true
                        userId
                    }
                    type == Int::class.javaPrimitiveType -> deviceId
                    type == Boolean::class.javaPrimitiveType -> false
                    else -> null
                }
            }
            .toTypedArray()
    }
}
