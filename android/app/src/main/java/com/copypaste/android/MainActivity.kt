package com.copypaste.android

import android.Manifest
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.util.Log
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.ContextCompat
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.History
import androidx.compose.material.icons.filled.QrCode
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.graphics.vector.ImageVector
import com.copypaste.android.ui.theme.CopyPasteTheme
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

/**
 * Main activity — launcher home + foreground clipboard listener (API 29+).
 *
 * The Android 10+ clipboard hook from the previous implementation is preserved;
 * a Compose UI now sits on top with quick links to:
 *   - History (last 50 clipboard items)
 *   - Pair Device (calls `startPairing()` stub)
 *   - Settings (sync toggle etc.)
 *
 * The background [ClipboardService] still handles API 26-28. Both paths share
 * the same pipeline: isSensitive -> encryptText -> store via
 * [ClipboardRepository].
 */
class MainActivity : ComponentActivity() {

    private lateinit var clipboardManager: ClipboardManager
    private lateinit var repository: ClipboardRepository
    private lateinit var settings: Settings
    private val scope = CoroutineScope(Dispatchers.Main)

    /**
     * v0.3 T4 polish: API 33+ requires the POST_NOTIFICATIONS runtime
     * permission before the foreground-service notification will render
     * with action buttons. Requested once at first launch; declined is fine,
     * the service still runs (just silently).
     */
    private val notificationPermissionLauncher = registerForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { granted ->
        Log.d(TAG, "POST_NOTIFICATIONS granted=$granted")
    }

    private val clipListener = ClipboardManager.OnPrimaryClipChangedListener {
        val clip = clipboardManager.primaryClip ?: return@OnPrimaryClipChangedListener
        val text = clip.getItemAt(0)?.text?.toString() ?: return@OnPrimaryClipChangedListener

        scope.launch(Dispatchers.IO) {
            handleClipboardChange(text)
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        settings = Settings(this)
        repository = ClipboardRepository(this)
        clipboardManager = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager

        // Android 10+ (API 29+): clipboard only readable in foreground
        clipboardManager.addPrimaryClipChangedListener(clipListener)

        maybeRequestNotificationPermission()
        startClipboardServiceIfPossible()

        setContent {
            CopyPasteTheme {
                MainScreen()
            }
        }
    }

    /**
     * Kick off [ClipboardService] so background capture continues on API 26-28
     * after the user leaves MainActivity. Wrapped in try/catch because
     * `startForegroundService` can throw [SecurityException] under heavy
     * background restrictions (API 31+) and
     * [android.app.ForegroundServiceStartNotAllowedException] (API 34+) when
     * the app is in a state that disallows foreground promotion. A failure
     * here is non-fatal: the in-activity clipListener still works while the
     * UI is visible.
     */
    private fun startClipboardServiceIfPossible() {
        try {
            val intent = Intent(this, ClipboardService::class.java)
            ContextCompat.startForegroundService(this, intent)
            Log.d(TAG, "ClipboardService start requested")
        } catch (e: Exception) {
            Log.w(TAG, "ClipboardService start failed: ${e.javaClass.simpleName} ${e.message}")
        }
    }

    override fun onDestroy() {
        clipboardManager.removePrimaryClipChangedListener(clipListener)
        super.onDestroy()
    }

    /**
     * Request POST_NOTIFICATIONS on API 33+ so the foreground service
     * notification (with Pause/Resume + Open actions) renders. Skipped on
     * older releases — permission is granted implicitly there.
     *
     * If the user denies it the service still runs; only the notification
     * channel is silenced.
     */
    private fun maybeRequestNotificationPermission() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return
        val granted = ContextCompat.checkSelfPermission(
            this,
            Manifest.permission.POST_NOTIFICATIONS
        ) == PackageManager.PERMISSION_GRANTED
        if (granted) return
        notificationPermissionLauncher.launch(Manifest.permission.POST_NOTIFICATIONS)
    }

    /**
     * Encrypt and store a clipboard change.
     * 1. Check sensitivity via UniFFI [isSensitive], fallback to false.
     * 2. Encrypt via UniFFI [encryptText], falling back to local AES-GCM.
     * 3. Persist via [ClipboardRepository.storeItem].
     * 4. Show a toast on the main thread when content is sensitive.
     */
    private suspend fun handleClipboardChange(text: String) {
        if (text.isBlank()) return

        val sensitive = try { isSensitive(text) } catch (_: UnsatisfiedLinkError) { false }

        if (sensitive && settings.showSensitiveWarnings) {
            runOnUiThread {
                Toast.makeText(this, "Sensitive data detected — not stored", Toast.LENGTH_SHORT).show()
            }
            Log.d(TAG, "Sensitive clip in MainActivity — skipped")
            return
        }

        val key = settings.encryptionKey
        val stored = repository.storeItem(text, key)
        if (stored) {
            Log.d(TAG, "Clipboard item stored from MainActivity")
        }
    }

    companion object {
        private const val TAG = "MainActivity"
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun MainScreen() {
    val ctx = LocalContext.current
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.app_name)) },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.primary,
                    titleContentColor = MaterialTheme.colorScheme.onPrimary,
                )
            )
        }
    ) { innerPadding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding)
                .padding(24.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(16.dp, Alignment.Top)
        ) {
            Text(
                text = stringResource(R.string.home_tagline),
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurface
            )
            NavButton(
                label = stringResource(R.string.nav_history),
                icon = Icons.Filled.History
            ) { ctx.startActivity(Intent(ctx, HistoryActivity::class.java)) }

            NavButton(
                label = stringResource(R.string.nav_pair),
                icon = Icons.Filled.QrCode
            ) { ctx.startActivity(Intent(ctx, PairActivity::class.java)) }

            NavButton(
                label = stringResource(R.string.nav_settings),
                icon = Icons.Filled.Settings
            ) { ctx.startActivity(Intent(ctx, SettingsActivity::class.java)) }
        }
    }
}

@Composable
private fun NavButton(label: String, icon: ImageVector, onClick: () -> Unit) {
    Button(
        onClick = onClick,
        modifier = Modifier.fillMaxWidth()
    ) {
        Icon(icon, contentDescription = null)
        Text(text = label, modifier = Modifier.padding(start = 8.dp))
    }
}
