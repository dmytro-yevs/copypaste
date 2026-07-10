package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Unit tests for [OemAutoStartHelper.detectManufacturer] — the pure
 * `Build.MANUFACTURER` -> [OemAutoStartHelper.Manufacturer] selector that
 * decides which OEM-specific autostart intent (component/action) gets built
 * for a device (S10 Wave E, CopyPaste-myh8.10). No Context/PackageManager
 * involved, so it is testable in the :app JVM test module without Robolectric.
 */
class OemAutoStartHelperManufacturerTest {

    @Test
    fun `xiaomi variants map to XIAOMI`() {
        assertEquals(OemAutoStartHelper.Manufacturer.XIAOMI, OemAutoStartHelper.detectManufacturer("Xiaomi"))
        assertEquals(OemAutoStartHelper.Manufacturer.XIAOMI, OemAutoStartHelper.detectManufacturer("POCO"))
        assertEquals(OemAutoStartHelper.Manufacturer.XIAOMI, OemAutoStartHelper.detectManufacturer("Redmi"))
    }

    @Test
    fun `huawei and honor map to HUAWEI`() {
        assertEquals(OemAutoStartHelper.Manufacturer.HUAWEI, OemAutoStartHelper.detectManufacturer("HUAWEI"))
        assertEquals(OemAutoStartHelper.Manufacturer.HUAWEI, OemAutoStartHelper.detectManufacturer("HONOR"))
    }

    @Test
    fun `genuine oppo and realme map to OPPO`() {
        assertEquals(OemAutoStartHelper.Manufacturer.OPPO, OemAutoStartHelper.detectManufacturer("OPPO"))
        assertEquals(OemAutoStartHelper.Manufacturer.OPPO, OemAutoStartHelper.detectManufacturer("realme"))
    }

    @Test
    fun `oneplus on an oppo ColorOS base maps to OPPO, not ONEPLUS`() {
        assertEquals(
            OemAutoStartHelper.Manufacturer.OPPO,
            OemAutoStartHelper.detectManufacturer("oneplus oppo colorOS"),
        )
    }

    @Test
    fun `genuine oneplus (OxygenOS, no oppo token) maps to ONEPLUS`() {
        assertEquals(OemAutoStartHelper.Manufacturer.ONEPLUS, OemAutoStartHelper.detectManufacturer("OnePlus"))
    }

    @Test
    fun `vivo and iqoo map to VIVO`() {
        assertEquals(OemAutoStartHelper.Manufacturer.VIVO, OemAutoStartHelper.detectManufacturer("vivo"))
        assertEquals(OemAutoStartHelper.Manufacturer.VIVO, OemAutoStartHelper.detectManufacturer("iQOO"))
    }

    @Test
    fun `samsung maps to SAMSUNG`() {
        assertEquals(OemAutoStartHelper.Manufacturer.SAMSUNG, OemAutoStartHelper.detectManufacturer("samsung"))
    }

    @Test
    fun `unrecognized manufacturer (e_g_ stock Pixel) maps to UNKNOWN`() {
        assertEquals(OemAutoStartHelper.Manufacturer.UNKNOWN, OemAutoStartHelper.detectManufacturer("Google"))
    }

    @Test
    fun `meizu and htc map to their own buckets, not UNKNOWN`() {
        assertEquals(OemAutoStartHelper.Manufacturer.MEIZU, OemAutoStartHelper.detectManufacturer("Meizu"))
        assertEquals(OemAutoStartHelper.Manufacturer.HTC, OemAutoStartHelper.detectManufacturer("HTC"))
    }
}
