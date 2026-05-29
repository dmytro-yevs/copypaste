package com.copypaste.android

import com.copypaste.android.OemAutoStartHelper.Manufacturer
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Pure-JVM unit tests for the OEM-selection logic in [OemAutoStartHelper].
 *
 * [OemAutoStartHelper.detectManufacturer] is a pure function of the raw
 * `Build.MANUFACTURER` string (no Android APIs), so it can be exercised on the
 * host JVM without an emulator. This guards the manufacturer-bucket mapping
 * that decides WHICH OEM autostart-settings intent the app will try first —
 * the heart of the BUG-1 fix.
 *
 * NOTE: these tests could not be executed in the worktree (no Android SDK /
 * Gradle available here); they are written to run via
 * `./gradlew :app:testDebugUnitTest` on a machine with the SDK.
 */
class OemAutoStartHelperTest {

    @Test
    fun xiaomi_family_maps_to_xiaomi() {
        assertEquals(Manufacturer.XIAOMI, OemAutoStartHelper.detectManufacturer("Xiaomi"))
        assertEquals(Manufacturer.XIAOMI, OemAutoStartHelper.detectManufacturer("xiaomi"))
        assertEquals(Manufacturer.XIAOMI, OemAutoStartHelper.detectManufacturer("POCO"))
        assertEquals(Manufacturer.XIAOMI, OemAutoStartHelper.detectManufacturer("Redmi"))
    }

    @Test
    fun huawei_and_honor_map_to_huawei() {
        assertEquals(Manufacturer.HUAWEI, OemAutoStartHelper.detectManufacturer("HUAWEI"))
        assertEquals(Manufacturer.HUAWEI, OemAutoStartHelper.detectManufacturer("Honor"))
    }

    @Test
    fun oppo_realme_map_to_oppo() {
        assertEquals(Manufacturer.OPPO, OemAutoStartHelper.detectManufacturer("OPPO"))
        assertEquals(Manufacturer.OPPO, OemAutoStartHelper.detectManufacturer("realme"))
    }

    @Test
    fun genuine_oneplus_maps_to_oneplus_not_oppo() {
        // A genuine OnePlus (OxygenOS) must keep its own bucket, not be swallowed
        // by the Oppo/ColorOS branch — only OnePlus models that ALSO report oppo
        // in the manufacturer string fall through to OPPO.
        assertEquals(Manufacturer.ONEPLUS, OemAutoStartHelper.detectManufacturer("OnePlus"))
    }

    @Test
    fun oneplus_on_coloros_base_maps_to_oppo() {
        assertEquals(Manufacturer.OPPO, OemAutoStartHelper.detectManufacturer("OnePlus (OPPO)"))
    }

    @Test
    fun vivo_and_iqoo_map_to_vivo() {
        assertEquals(Manufacturer.VIVO, OemAutoStartHelper.detectManufacturer("vivo"))
        assertEquals(Manufacturer.VIVO, OemAutoStartHelper.detectManufacturer("iQOO"))
    }

    @Test
    fun samsung_asus_letv_nokia_meizu_htc_map_correctly() {
        assertEquals(Manufacturer.SAMSUNG, OemAutoStartHelper.detectManufacturer("samsung"))
        assertEquals(Manufacturer.ASUS, OemAutoStartHelper.detectManufacturer("asus"))
        assertEquals(Manufacturer.LETV, OemAutoStartHelper.detectManufacturer("LeEco"))
        assertEquals(Manufacturer.NOKIA, OemAutoStartHelper.detectManufacturer("HMD Global Nokia"))
        assertEquals(Manufacturer.MEIZU, OemAutoStartHelper.detectManufacturer("Meizu"))
        assertEquals(Manufacturer.HTC, OemAutoStartHelper.detectManufacturer("HTC"))
    }

    @Test
    fun unknown_oem_maps_to_unknown() {
        assertEquals(Manufacturer.UNKNOWN, OemAutoStartHelper.detectManufacturer("Google"))
        assertEquals(Manufacturer.UNKNOWN, OemAutoStartHelper.detectManufacturer("Pixel"))
        assertEquals(Manufacturer.UNKNOWN, OemAutoStartHelper.detectManufacturer(""))
    }
}
