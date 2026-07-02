package com.copypaste.android

import android.content.Intent
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.unit.dp
import com.copypaste.android.ui.GlassToastHost
import com.copypaste.android.ui.GlassToastKind
import com.copypaste.android.ui.GlassToastState
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.CopyPasteCard
import com.copypaste.android.ui.theme.CopyPasteTopBar
import com.copypaste.android.ui.theme.CpTypography
import com.copypaste.android.ui.theme.LocalCpColors
import com.copypaste.android.ui.theme.SectionLabel
import kotlinx.coroutines.launch

// ─────────────────────────────────────────────────────────────────────────────
// Composable screen (also embedded in MainShell's DEVICES tab)
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Devices screen — shows the full roster of paired P2P peers, each as a card
 * with a real-presence online dot, model, OS, version, IP fields, last-sync time,
 * and per-peer Unpair / Revoke actions. Parity with the macOS DevicesView.
 *
 * Navigation: launched from the DEVICES tab in [MainActivity] bottom nav, and
 * also accessible as a standalone activity from [SettingsActivity] (General tab
 * "Devices" row).
 *
 * CopyPaste-vp63.39: discovery/pairing/unpair/revoke state + logic now lives in
 * [DevicesController] (obtained via [rememberDevicesController]); the dialog
 * cluster lives in [DevicesDialogs]. This composable is list orchestration only
 * — the own-device/paired-peer/discovered-peer rows and the Scaffold shell.
 */
@Composable
fun DevicesScreen(
    modifier: Modifier = Modifier,
    showBackButton: Boolean = true,
    onBack: () -> Unit = {},
    /**
     * When true (set by tapping the incoming-pair notification), the screen
     * immediately polls [pairGetSas] once on composition and auto-opens the SAS
     * modal if the state is `awaiting_sas`. Consumed after the first check.
     */
    autoOpenSasOnEntry: Boolean = false,
    /** §1: paint the canvas backdrop here (standalone) vs. via MainShell (embedded). */
    paintCanvasBackdrop: Boolean = true,
) {
    val ctx = LocalContext.current
    val settings = remember { Settings(ctx) }
    val deviceKeyStore = remember { DeviceKeyStore(ctx) }
    val controller = rememberDevicesController(
        settings = settings,
        deviceKeyStore = deviceKeyStore,
        autoOpenSasOnEntry = autoOpenSasOnEntry,
    )
    val cp = LocalCpColors.current

    // android-devices spec "Fingerprint tap-to-copy parity" — mirrors
    // PairScreen's copyFingerprint()/GlassToastHost pattern (S8): PeerRow/
    // OwnDeviceRow have no LocalClipboardManager dependency of their own, so
    // the actual clipboard write + toast lives here, one level up.
    val scope = rememberCoroutineScope()
    val clipboardManager = LocalClipboardManager.current
    val toastState = remember { GlassToastState() }
    val copiedMessage = stringResource(R.string.devices_fingerprint_copied)
    fun copyFingerprint(fingerprint: String) {
        clipboardManager.setText(AnnotatedString(fingerprint))
        scope.launch {
            toastState.show(copiedMessage, GlassToastKind.ACCENT)
        }
    }

    // The full dialog set (unpair/revoke/revoke-rotate/revoke-error/revoke-all/
    // SAS pairing/scan error) — data-driven off [controller]'s state.
    DevicesDialogs(controller = controller, settings = settings)

    Box(modifier = Modifier.fillMaxSize()) {
    Scaffold(
        modifier = modifier,
        topBar = {
            CopyPasteTopBar(
                title = stringResource(R.string.title_devices),
                showBackButton = showBackButton,
                onBack = onBack,
                backContentDescription = "Back",
                // CopyPaste-crh3.34: "Revoke all" action mirrors macOS DevicesView
                // actions bar. DANGER variant matches macOS border-ide-danger/35 styling
                // (STYLEGUIDE §9.1). Disabled when no peers or an operation is in flight.
                actions = {
                    CopyPasteButton(
                        onClick = { controller.revoke.openRevokeAllConfirm() },
                        variant = ButtonVariant.DANGER,
                        enabled = revokeAllEnabled(controller.peers.size) && !controller.revoke.revokeAllInFlight,
                        modifier = Modifier.padding(end = 8.dp),
                    ) {
                        Text(stringResource(R.string.devices_btn_revoke_all_confirm))
                    }
                },
            )
        },
    ) { innerPadding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 16.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {

            // ── Deliverable 1: own QR at the top, always visible, blurred ────
            // Shows THIS device's pairing QR at the top of the screen so the
            // user doesn't need to navigate to PairActivity to get scanned.
            // The QR is blurred by default (tap to reveal) because it encodes
            // the PAKE password + sync provisioning material. Reuses the same
            // blur/reveal pattern as PairActivity (Modifier.blur(16.dp) + overlay
            // label, first-tap reveals, second-tap regenerates and stays visible).
            // The QR is generated lazily in OwnQrSection. DevicesActivity now sets
            // FLAG_SECURE in onCreate (CopyPaste-92qs), so the reveal flow here is
            // screenshot-protected just like PairActivity's; the blur-at-rest is a
            // second layer of defence.
            OwnQrSection(settings = settings)

            // ── Single grouped inset device list (PARITY-SPEC §8) ─────────────
            // Apple Settings-style: this device first, then every paired peer,
            // then discovered (unpaired) LAN peers — ALL inside ONE glass
            // CopyPasteCard, rows separated by a single 1dp hairline divider.
            // Replaces the former stack of individually-elevated Cards.
            // CopyPaste-9ln4: renamed from "Devices" to "Paired devices" — avoids
            // duplicate with the TopBar title and matches the web SectionLabel fix.
            // bdac.48: sentence case to match all other section headers on this screen.
            SectionLabel(stringResource(R.string.devices_section_paired))

            // CopyPaste-crh3.30: the device's active secondary (non-P2P) transport,
            // mirroring the macOS daemon's relay>supabase priority. Read from the
            // live sync config so a cloud-only peer is labelled Relay vs Cloud
            // instead of collapsing both into "Cloud".
            val cloudTransport = activeCloudTransport(
                relayActive = settings.relayEnabled && settings.isRelayConfigured,
                supabaseActive = settings.supabaseEnabled && settings.isSupabaseConfigured,
            )

            // Assemble the ordered row list so we know where dividers go (a
            // divider is drawn BEFORE every row except the first).
            val deviceRows: List<@Composable () -> Unit> = buildList {
                // This device — always first.
                controller.ownIdentity?.let { identity ->
                    add {
                        OwnDeviceRow(
                            identity = identity,
                            nowMs = controller.nowMs,
                            ownPublicIp = controller.ownPublicIp,
                            onCopyFingerprint = ::copyFingerprint,
                        )
                    }
                }
                // Paired peers — pass the pre-computed online flag so the row dot
                // and the footer badge are always in sync.
                for (peer in controller.peers) {
                    add {
                        PeerRow(
                            peer = peer,
                            online = controller.onlineByFingerprint[peer.fingerprint] ?: false,
                            nowMs = controller.nowMs,
                            cloudTransport = cloudTransport,
                            onUnpair = { controller.revoke.unpairTarget = peer },
                            onRevoke = { controller.revoke.revokeTarget = peer },
                            onCopyFingerprint = ::copyFingerprint,
                        )
                    }
                }
                // Discovered (unpaired) LAN peers — only when P2P is enabled
                // (discovery is gated on it). Always show the section label + an
                // empty-state row while scanning so the LAN feature stays visible
                // instead of silently vanishing (pkd0 regression). RowDivider
                // between rows is added by the forEachIndexed renderer below.
                if (controller.p2pEnabled) {
                    add {
                        // 1jms.20: use SectionLabel for visual consistency with all other
                        // section headers (Paired Devices, Your QR code, etc.).
                        SectionLabel(stringResource(R.string.devices_discovered_section))
                    }
                    if (controller.discovered.isEmpty()) {
                        // CopyPaste-0nd4: add DiscoveryRingsIcon + text in a Row so the
                        // empty-state has an icon anchor and visual breathing room, matching
                        // the macOS .network-rings icon + text pattern in DevicesView.tsx.
                        // "Scanning" state (android-devices spec) — distinct from the
                        // no-paired-peers empty state rendered below via [NoPeerCard].
                        add {
                            Row(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .padding(horizontal = 16.dp, vertical = 12.dp),
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.spacedBy(12.dp),
                            ) {
                                Text(
                                    text = stringResource(R.string.no_devices_nearby),
                                    style = CpTypography.meta,
                                    color = cp.faint,
                                )
                            }
                        }
                    } else {
                        for (peer in controller.discovered) {
                            add {
                                DiscoveredPeerRow(
                                    peer = peer,
                                    busy = controller.pairStarting || controller.pairingPeer != null,
                                    onPair = { controller.startPairing(peer) },
                                )
                            }
                        }
                    }
                }
            }

            if (deviceRows.isNotEmpty()) {
                CopyPasteCard {
                    // STYLEGUIDE §3.2: rows separated by a single hairline divider.
                    deviceRows.forEachIndexed { index, row ->
                        if (index > 0) {
                            RowDivider()
                        }
                        row()
                    }
                }
            }
            // §9.10 empty state — driven purely by the paired-roster count, NOT
            // by [deviceRows] (which also carries the own-device row and the
            // unrelated LAN-discovery section). android-devices spec "Empty
            // state when no peers are paired": renders alongside the own-device
            // card above when present, and stands alone (old fallback shape)
            // when [DevicesController.ownIdentity] is also unresolved.
            if (controller.peers.isEmpty()) {
                NoPeerCard(
                    onPair = {
                        ctx.startActivity(Intent(ctx, PairActivity::class.java))
                    }
                )
            }

            if (controller.p2pEnabled) {
                controller.discoverError?.let { msg ->
                    Text(
                        text = msg,
                        style = CpTypography.meta,
                        color = cp.err,
                    )
                }
            }

            // ── Deliverable 2: Scan button opens the camera directly ─────────
            // Launches PortraitCaptureActivity (ZXing) via ScanContract without
            // routing through PairActivity. The scan result is forwarded to
            // PairActivity as a cppair:// deep-link so PAKE + provisioning still
            // run there unmodified.
            // CopyPaste-jkbo: replaced raw OutlinedButton with shared CopyPasteButton(SECONDARY).
            CopyPasteButton(
                onClick = { controller.startScanFlow() },
                variant = ButtonVariant.SECONDARY,
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text(stringResource(R.string.btn_scan_qr))
            }

            Spacer(Modifier.height(24.dp))
        }
    }
    // android-devices spec "Fingerprint tap-to-copy parity" — copy feedback.
    GlassToastHost(state = toastState)
    }
}
