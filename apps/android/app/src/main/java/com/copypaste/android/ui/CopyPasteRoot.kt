package com.copypaste.android.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Devices
import androidx.compose.material.icons.filled.History
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import com.copypaste.android.R
import com.copypaste.android.data.ClipboardRepository
import com.copypaste.android.data.friendlyMessage
import com.copypaste.android.service.SyncService
import com.copypaste.android.ui.devices.DevicesScreen
import com.copypaste.android.ui.devices.DevicesViewModel
import com.copypaste.android.ui.history.HistoryScreen
import com.copypaste.android.ui.history.HistoryViewModel
import com.copypaste.android.ui.settings.SettingsScreen
import com.copypaste.android.ui.theme.CopyPasteTheme

/** The three destinations. */
private enum class Destination(val label: Int, val icon: ImageVector) {
    History(R.string.nav_history, Icons.Filled.History),
    Devices(R.string.nav_devices, Icons.Filled.Devices),
    Settings(R.string.nav_settings, Icons.Filled.Settings),
}

/**
 * The app shell.
 *
 * Manifest 06 INV-20: the shell is never inside a failure boundary. Opening the
 * core can fail — a Keystore key lost to a device restore is the realistic case
 * — and when it does, [FailedToOpen] renders *inside* this scaffold rather than
 * against a bare window, so the user still has an app rather than a blank
 * screen with a sentence on it.
 */
@Composable
fun CopyPasteRoot(
    repository: Result<ClipboardRepository>,
    notificationsGranted: Boolean,
    onRequestNotifications: () -> Unit,
    sharedText: String?,
) {
    CopyPasteTheme {
        var destination by rememberSaveable { mutableStateOf(Destination.History) }

        Scaffold(
            bottomBar = {
                NavigationBar {
                    Destination.entries.forEach { entry ->
                        NavigationBarItem(
                            selected = destination == entry,
                            onClick = { destination = entry },
                            icon = {
                                // A11Y-9: the icon is decorative; the item's
                                // name comes from its label, and Material 3's
                                // NavigationBarItem exposes selected state as
                                // `selected` for the accessibility tree.
                                Icon(entry.icon, contentDescription = null)
                            },
                            label = { Text(stringResource(entry.label)) },
                        )
                    }
                }
            },
        ) { padding ->
            val repo = repository.getOrNull()
            if (repo == null) {
                FailedToOpen(
                    error = repository.exceptionOrNull(),
                    modifier = Modifier.fillMaxSize().padding(padding),
                )
                return@Scaffold
            }

            when (destination) {
                Destination.History -> HistoryPane(repo, sharedText, Modifier.padding(padding))
                Destination.Devices -> DevicesPane(repo, Modifier.padding(padding))
                Destination.Settings -> SettingsPane(
                    notificationsGranted = notificationsGranted,
                    onRequestNotifications = onRequestNotifications,
                    modifier = Modifier.padding(padding),
                )
            }
        }
    }
}

@Composable
private fun HistoryPane(
    repo: ClipboardRepository,
    sharedText: String?,
    modifier: Modifier,
) {
    val viewModel: HistoryViewModel = viewModel(factory = factory { HistoryViewModel(repo) })

    // Text that arrived from the share sheet or the selection menu. Keyed so a
    // recomposition does not store it twice.
    androidx.compose.runtime.LaunchedEffect(sharedText) {
        sharedText?.let(viewModel::addExternal)
    }

    // INV-11, trigger two of two: revealed content re-hides when the app stops
    // being the thing in front of the user. Independent of the 10 s timer in
    // HistoryScreen — either alone is a gap.
    val lifecycleOwner = LocalLifecycleOwner.current
    DisposableEffect(lifecycleOwner) {
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_PAUSE) viewModel.hideRevealed()
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
    }

    HistoryScreen(viewModel = viewModel, modifier = modifier)
}

@Composable
private fun DevicesPane(repo: ClipboardRepository, modifier: Modifier) {
    val viewModel: DevicesViewModel = viewModel(factory = factory { DevicesViewModel(repo) })

    // The pairing code stops being on screen the moment the app does. It is a
    // live credential and the recent-apps thumbnail is a surface we do not draw
    // (FLAG_SECURE covers the screenshot; this covers the state).
    val lifecycleOwner = LocalLifecycleOwner.current
    DisposableEffect(lifecycleOwner) {
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_PAUSE) viewModel.dismissPairingCode()
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
    }

    DevicesScreen(viewModel = viewModel, modifier = modifier)
}

@Composable
private fun SettingsPane(
    notificationsGranted: Boolean,
    onRequestNotifications: () -> Unit,
    modifier: Modifier,
) {
    val context = LocalContext.current
    var captureOnOpen by rememberSaveable { mutableStateOf(true) }
    var serviceRunning by rememberSaveable { mutableStateOf(false) }

    SettingsScreen(
        captureOnOpen = captureOnOpen,
        onCaptureOnOpenChange = { captureOnOpen = it },
        serviceRunning = serviceRunning,
        onServiceRunningChange = { wanted ->
            when {
                !wanted -> {
                    SyncService.stop(context)
                    serviceRunning = false
                }
                // A foreground service whose notification is suppressed is a
                // background service the user cannot see or stop. Ask first;
                // if the answer is no, do not start it.
                !notificationsGranted -> onRequestNotifications()
                else -> {
                    SyncService.start(context)
                    serviceRunning = true
                }
            }
        },
        modifier = modifier,
    )
}

/**
 * The core would not open.
 *
 * Renders the friendly sentence for whatever went wrong — never the throwable's
 * own text (INV-12) — and offers nothing destructive. The realistic cause is a
 * Keystore key lost to a restore onto new hardware, and the encrypted database
 * is still on disk: an "erase and start over" button one tap away from a user
 * who is already confused is how that history gets thrown away for good.
 */
@Composable
private fun FailedToOpen(error: Throwable?, modifier: Modifier = Modifier) {
    Box(modifier.padding(32.dp), contentAlignment = Alignment.Center) {
        Column {
            Text(
                text = stringResource(R.string.open_failed_title),
                style = MaterialTheme.typography.titleMedium,
            )
            Text(
                text = stringResource(friendlyMessage(error ?: IllegalStateException())),
                style = MaterialTheme.typography.bodyMedium,
                modifier = Modifier.padding(top = 8.dp),
            )
        }
    }
}

/** A one-off `ViewModelProvider.Factory`, so the repository can be injected. */
private inline fun <reified T : ViewModel> factory(
    crossinline create: () -> T,
): ViewModelProvider.Factory = object : ViewModelProvider.Factory {
    @Suppress("UNCHECKED_CAST")
    override fun <V : ViewModel> create(modelClass: Class<V>): V = create() as V
}
