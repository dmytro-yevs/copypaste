package com.copypaste.app

internal object TileAddGate {
    const val TILE_NOT_ADDED = 0
    const val TILE_ALREADY_ADDED = 1
    const val TILE_ADDED = 2

    fun status(result: Int): String =
        if (result == TILE_ADDED || result == TILE_ALREADY_ADDED) "granted" else "denied"
}
