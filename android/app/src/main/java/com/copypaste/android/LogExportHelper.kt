package com.copypaste.android

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.widget.Toast
import androidx.core.content.FileProvider
import java.io.File
import java.io.FileOutputStream
import java.util.zip.ZipEntry
import java.util.zip.ZipOutputStream

/**
 * Bundles all log files into a single zip and fires a Share intent so
 * the user can send them via email, Drive, Slack, etc.
 *
 * Uses FileProvider (authority derived from `${context.packageName}.fileprovider`,
 * matching HistoryActivity) to produce a content:// URI that is safe to pass to
 * third-party apps.
 *
 * The zip is written to internal cache (not the external log dir) so it is
 * automatically cleaned up by the OS and is never accidentally adb-pulled.
 */
object LogExportHelper {

    private const val TAG = "LogExportHelper"

    /**
     * Zip all log files and start a chooser Share intent.
     * Call from any Activity or Context that can start activities.
     *
     * @param context must be an Activity context (or have FLAG_ACTIVITY_NEW_TASK).
     */
    fun shareLogsZip(context: Context) {
        val files = AppLogger.allLogFiles(context)
        if (files.isEmpty()) {
            Toast.makeText(context, context.getString(R.string.log_export_empty), Toast.LENGTH_SHORT).show()
            return
        }

        val zipFile = buildZip(context, files) ?: run {
            Toast.makeText(context, context.getString(R.string.log_export_failed), Toast.LENGTH_SHORT).show()
            return
        }

        val uri: Uri = try {
            // Derive the authority from the package name (matches HistoryActivity)
            // so a build-variant applicationId suffix can't desync it.
            FileProvider.getUriForFile(context, "${context.packageName}.fileprovider", zipFile)
        } catch (e: Exception) {
            AppLogger.e(TAG, "FileProvider failed", e)
            Toast.makeText(context, context.getString(R.string.log_export_failed), Toast.LENGTH_SHORT).show()
            return
        }

        val shareIntent = Intent(Intent.ACTION_SEND).apply {
            type = "application/zip"
            putExtra(Intent.EXTRA_STREAM, uri)
            putExtra(Intent.EXTRA_SUBJECT, "CopyPaste logs")
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }

        val chooser = Intent.createChooser(shareIntent, context.getString(R.string.log_export_chooser_title))
        chooser.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        context.startActivity(chooser)
    }

    // ── Internals ────────────────────────────────────────────────────────────

    private fun buildZip(context: Context, files: List<File>): File? {
        return try {
            val cacheDir = File(context.cacheDir, "log_export").also { it.mkdirs() }
            val zipFile = File(cacheDir, "copypaste_logs.zip")
            ZipOutputStream(FileOutputStream(zipFile)).use { zos ->
                for (f in files) {
                    zos.putNextEntry(ZipEntry(f.name))
                    f.inputStream().use { it.copyTo(zos) }
                    zos.closeEntry()
                }
            }
            zipFile
        } catch (e: Exception) {
            AppLogger.e(TAG, "Failed to build log zip", e)
            null
        }
    }
}
