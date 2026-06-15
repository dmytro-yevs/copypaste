package com.copypaste.android

import android.util.Log
import kotlinx.coroutines.withTimeout
import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.InetAddress

/**
 * Minimal STUN utility for discovering the device's WAN/public IPv4 address.
 *
 * Analogous to `public_ip.rs` in the Rust daemon:
 *   - Sends a STUN Binding Request to stun.l.google.com:19302 (Google's public STUN server).
 *   - Parses the XOR-MAPPED-ADDRESS attribute from the response.
 *   - Returns the public IPv4 as a String, or null on failure / when the setting is disabled.
 *
 * This is a one-shot, non-persistent query. The result is shown on the own-device card
 * (OwnDeviceRow) and is NEVER sent to analytics (matches the macOS setting gate).
 *
 * STUN wire format (RFC 5389):
 *   Message header (20 bytes):
 *     [0-1]  Message Type (0x0001 = Binding Request)
 *     [2-3]  Message Length (0 for a bare request)
 *     [4-7]  Magic Cookie (0x2112A442)
 *     [8-19] Transaction ID (12 random bytes)
 *   Response attribute type 0x0020 = XOR-MAPPED-ADDRESS:
 *     [0-1]  Type  (0x0020)
 *     [2-3]  Length
 *     [4]    (reserved)
 *     [5]    Family (0x01 = IPv4)
 *     [6-7]  X-Port (Port XOR'd with magic cookie MSW)
 *     [8-11] X-Address (IPv4 XOR'd with magic cookie)
 *
 * CopyPaste-6qq1.
 */
object StunUtils {

    private const val TAG = "StunUtils"
    private const val STUN_HOST = "stun.l.google.com"
    private const val STUN_PORT = 19302
    private const val TIMEOUT_MS = 5_000L   // 5-second total budget (coroutine + socket)
    private const val SOCKET_TIMEOUT_MS = 4_500  // slightly under coroutine budget
    private const val RESPONSE_BUFFER_SIZE = 512

    // STUN magic cookie (RFC 5389 §6)
    private const val MAGIC_COOKIE = 0x2112A442.toInt()

    /**
     * Query the public (WAN) IPv4 address via a STUN Binding Request.
     *
     * @param collectEnabled mirrors [Settings.collectPublicIp] — returns null immediately
     *   when the user has not opted in (parity with macOS STUN gate in public_ip.rs).
     * @return the public IPv4 string (e.g. "203.0.113.45"), or null on failure/disabled.
     */
    suspend fun queryPublicIp(collectEnabled: Boolean): String? {
        if (!collectEnabled) return null

        return try {
            withTimeout(TIMEOUT_MS) {
                queryPublicIpBlocking()
            }
        } catch (e: Exception) {
            Log.d(TAG, "STUN query failed: ${e.message}")
            null
        }
    }

    /**
     * Blocking STUN implementation — must be called from an IO dispatcher
     * (the [queryPublicIp] caller uses withContext(Dispatchers.IO)).
     */
    private fun queryPublicIpBlocking(): String? {
        // Build a minimal STUN Binding Request (20-byte header, no attributes).
        val request = ByteArray(20)
        // Message Type: 0x0001 (Binding Request)
        request[0] = 0x00
        request[1] = 0x01
        // Message Length: 0 (no attributes)
        request[2] = 0x00
        request[3] = 0x00
        // Magic Cookie: 0x2112A442
        request[4] = 0x21
        request[5] = 0x12
        request[6] = 0xA4.toByte()
        request[7] = 0x42
        // Transaction ID: 12 bytes (random, not validated — we match any response)
        val txId = java.util.UUID.randomUUID()
        val txBytes = ByteArray(16)
        val bb = java.nio.ByteBuffer.wrap(txBytes)
        bb.putLong(txId.mostSignificantBits)
        bb.putLong(txId.leastSignificantBits)
        System.arraycopy(txBytes, 4, request, 8, 12)

        DatagramSocket().use { socket ->
            socket.soTimeout = SOCKET_TIMEOUT_MS
            val serverAddr = InetAddress.getByName(STUN_HOST)
            val sendPacket = DatagramPacket(request, request.size, serverAddr, STUN_PORT)
            socket.send(sendPacket)

            val buf = ByteArray(RESPONSE_BUFFER_SIZE)
            val recvPacket = DatagramPacket(buf, buf.size)
            socket.receive(recvPacket)

            return parseXorMappedAddress(recvPacket.data, recvPacket.length)
        }
    }

    /**
     * Parse XOR-MAPPED-ADDRESS (type 0x0020) from a STUN Binding Response.
     * Scans the attribute list starting at byte 20 (after the 20-byte header).
     * Returns the IPv4 string on success, or null if the attribute is absent/malformed.
     */
    private fun parseXorMappedAddress(data: ByteArray, len: Int): String? {
        if (len < 20) return null
        // Verify Message Type = 0x0101 (Binding Response) or 0x0111 (Error)
        val msgType = ((data[0].toInt() and 0xFF) shl 8) or (data[1].toInt() and 0xFF)
        if (msgType != 0x0101) return null

        var offset = 20
        while (offset + 4 <= len) {
            val attrType = ((data[offset].toInt() and 0xFF) shl 8) or (data[offset + 1].toInt() and 0xFF)
            val attrLen  = ((data[offset + 2].toInt() and 0xFF) shl 8) or (data[offset + 3].toInt() and 0xFF)
            offset += 4

            if (attrType == 0x0020) {   // XOR-MAPPED-ADDRESS
                // Needs at least 8 bytes: 1 reserved + 1 family + 2 port + 4 address
                if (attrLen < 8 || offset + attrLen > len) return null
                val family = data[offset + 1].toInt() and 0xFF
                if (family != 0x01) return null  // only IPv4

                // XOR-MAPPED-ADDRESS IPv4: addr XOR'd with magic cookie big-endian
                val xAddr = ByteArray(4)
                xAddr[0] = (data[offset + 4].toInt() xor 0x21).toByte()
                xAddr[1] = (data[offset + 5].toInt() xor 0x12).toByte()
                xAddr[2] = (data[offset + 6].toInt() xor 0xA4).toByte()
                xAddr[3] = (data[offset + 7].toInt() xor 0x42).toByte()

                return InetAddress.getByAddress(xAddr).hostAddress
            }

            // Advance past this attribute (padded to 4-byte boundary)
            offset += attrLen
            if (attrLen % 4 != 0) offset += 4 - (attrLen % 4)
        }
        return null
    }
}
