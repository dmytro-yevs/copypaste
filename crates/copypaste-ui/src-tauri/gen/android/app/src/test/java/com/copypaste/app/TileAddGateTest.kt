package com.copypaste.app

import org.junit.Assert.assertEquals
import org.junit.Test

class TileAddGateTest {
    @Test
    fun addedAndAlreadyAddedAreGrants() {
        assertEquals("granted", TileAddGate.status(TileAddGate.TILE_ADDED))
        assertEquals("granted", TileAddGate.status(TileAddGate.TILE_ALREADY_ADDED))
        assertEquals("denied", TileAddGate.status(TileAddGate.TILE_NOT_ADDED))
    }
}
