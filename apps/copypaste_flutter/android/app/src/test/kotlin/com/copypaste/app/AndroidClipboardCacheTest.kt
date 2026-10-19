package com.copypaste.app

import android.content.ClipboardManager
import android.content.Context
import java.io.File
import java.util.UUID
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class AndroidClipboardCacheTest {
    @Before fun initializeProvider() = initializeClipboardTestProvider()

    @Test fun cleanupDuringPublicationProtectsBothPossibleOwnersAndFailureRestoresPreviousOwner() {
        val context = RuntimeEnvironment.getApplication()
        assertTrue(AndroidClipboardWriter.writeBinary(context, byteArrayOf(1, 2), "original.bin", "application/octet-stream"))
        val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        val uri = clipboard.primaryClip!!.getItemAt(0).uri
        val root = File(context.cacheDir, "clipboard")
        val previous = root.listFiles()!!.single { it.isDirectory }
        val candidate = File(File(root, UUID.randomUUID().toString()).apply { mkdirs() }, "candidate.bin")
        candidate.writeBytes(byteArrayOf(3, 4))
        val future = System.currentTimeMillis() + 3 * 24 * 60 * 60 * 1000L
        val result = runCatching {
            AndroidClipboardCache.publish(context, candidate) {
                AndroidClipboardCache.cleanup(context, future)
                assertTrue(previous.exists())
                assertTrue(candidate.exists())
                throw IllegalStateException("publication interrupted")
            }
        }
        assertTrue(result.isFailure)
        AndroidClipboardCache.cleanup(context, future)
        assertTrue(previous.exists())
        assertFalse(candidate.exists())
        assertArrayEquals(byteArrayOf(1, 2), context.contentResolver.openInputStream(uri)!!.use { it.readBytes() })
    }
    @Test fun obsoletePublicationsExpireButCurrentAndRecentUrisKeepTheirBytes() {
        val context = RuntimeEnvironment.getApplication()
        val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        val now = System.currentTimeMillis()
        val old = now - 3 * 24 * 60 * 60 * 1000L
        assertTrue(AndroidClipboardWriter.writeBinary(context, byteArrayOf(1, 2), "old.png", "image/png"))
        val first = clipboard.primaryClip!!.getItemAt(0).uri
        val root = File(context.cacheDir, "clipboard")
        val oldDir = root.listFiles()!!.single { it.isDirectory }
        assertTrue(oldDir.setLastModified(old))
        assertTrue(AndroidClipboardWriter.writeBinary(context, byteArrayOf(3, 4), "current.png", "image/png"))
        val current = clipboard.primaryClip!!.getItemAt(0).uri
        val currentDir = root.listFiles()!!.single { it.isDirectory && it != oldDir }
        assertTrue(currentDir.setLastModified(old))
        val recent = File(root, UUID.randomUUID().toString()).apply { mkdirs() }
        File(recent, "recent.bin").writeBytes(byteArrayOf(5))
        AndroidClipboardCache.cleanup(context, now)
        assertFalse(oldDir.exists())
        assertTrue(currentDir.exists())
        assertTrue(recent.exists())
        assertArrayEquals(byteArrayOf(3, 4), context.contentResolver.openInputStream(current)!!.use { it.readBytes() })
        assertNotEquals(first, current)
    }

    @Test fun unknownLegacyOwnerAndFailedPublicationRemainProtected() {
        val context = RuntimeEnvironment.getApplication()
        val root = File(context.cacheDir, "clipboard").apply { mkdirs() }
        root.deleteRecursively()
        root.mkdirs()
        val legacy = File(root, UUID.randomUUID().toString()).apply { mkdirs() }
        legacy.setLastModified(1)
        val oldShare = File(root, "copypaste-old-export.bin").apply { writeBytes(byteArrayOf(8)); setLastModified(1) }
        AndroidClipboardCache.cleanup(context, System.currentTimeMillis())
        assertTrue(legacy.exists())
        assertFalse(oldShare.exists())
        val staged = File(File(root, UUID.randomUUID().toString()).apply { mkdirs() }, "file.bin")
        staged.writeBytes(byteArrayOf(7))
        runCatching { AndroidClipboardCache.publish(context, staged) { error("rejected publication") } }
        AndroidClipboardCache.cleanup(context, System.currentTimeMillis())
        assertTrue(legacy.exists())
    }
}
