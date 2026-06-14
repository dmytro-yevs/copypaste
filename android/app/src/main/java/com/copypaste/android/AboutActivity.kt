@file:OptIn(ExperimentalMaterial3Api::class)

package com.copypaste.android

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.OpenInNew
import androidx.compose.material.icons.outlined.Check
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.copypaste.android.ui.theme.CopyPasteCard
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.CopyPasteTopBar
import com.copypaste.android.ui.theme.IdeAccent
import com.copypaste.android.ui.theme.IdeBg
import com.copypaste.android.ui.theme.IdeDim
import com.copypaste.android.ui.theme.IdeFaint
import com.copypaste.android.ui.theme.IdeSuccess
import com.copypaste.android.ui.theme.IdeText

/**
 * "About" screen — mirrors the macOS About view
 * (crates/copypaste-ui/src/views/AboutView.tsx):
 *
 *   • Identity block: app name, version string, one-line tagline.
 *   • Feature list (the same three bullets shown on macOS).
 *   • GitHub repository link (opens in the system browser).
 *
 * The version string is built from [BuildConfig.VERSION_NAME] and
 * [BuildConfig.VERSION_CODE] (generated from versionName / versionCode in
 * android/app/build.gradle.kts), rendered as "0.5.3 (build 8)". This is the
 * single source of truth the user can read on-device, and it can never drift
 * from the installed package the way a hardcoded string would.
 *
 * The macOS view additionally shows a live "Background daemon" status row.
 * Android has no separate daemon process (the foreground ClipboardService runs
 * in-process), so that row is intentionally omitted rather than faked.
 *
 * Reachable both as a standalone Activity and, primarily, as the "About" tab in
 * [MainActivity]'s bottom navigation via the [AboutScreen] composable.
 */
class AboutActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            CopyPasteTheme {
                Scaffold(containerColor = IdeBg) { innerPadding ->
                    AboutScreen(
                        modifier = Modifier.padding(innerPadding),
                        showBackButton = true,
                        onBack = { finish() },
                    )
                }
            }
        }
    }
}

/** Feature bullets — mirrors FEATURES in macOS AboutView.tsx. */
private val ABOUT_FEATURES = listOf(
    "End-to-end encrypted local history",
    "Peer-to-peer device sync",
    "Automatic sensitive-data redaction",
)

private const val GITHUB_URL = "https://github.com/dmytro-yevs/copypaste"

/** Human version string, e.g. "0.5.3 (build 8)". */
private fun versionLabel(): String =
    "${BuildConfig.VERSION_NAME} (build ${BuildConfig.VERSION_CODE})"

/**
 * About content, hostable inside [MainActivity]'s nav shell or the standalone
 * [AboutActivity]. Styled with the shared IDE-theme components
 * (CopyPasteTopBar / CopyPasteCard / IDE color tokens) so it reads as a sibling
 * of the History / Pair / Settings screens.
 */
@Composable
fun AboutScreen(
    modifier: Modifier = Modifier,
    showBackButton: Boolean = false,
    onBack: () -> Unit = {},
) {
    val context = LocalContext.current

    Scaffold(
        modifier = modifier,
        containerColor = IdeBg,
        topBar = {
            CopyPasteTopBar(
                title = stringResource(R.string.title_about),
                showBackButton = showBackButton,
                onBack = onBack,
            )
        },
    ) { innerPadding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding)
                .verticalScroll(rememberScrollState())
                .padding(16.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            CopyPasteCard {
                // ── Identity ─────────────────────────────────────────────
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 24.dp, vertical = 24.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Text(
                        text = stringResource(R.string.app_name),
                        style = MaterialTheme.typography.titleLarge,
                        fontWeight = FontWeight.SemiBold,
                        color = IdeText,
                    )
                    Spacer(Modifier.height(2.dp))
                    Text(
                        text = versionLabel(),
                        style = MaterialTheme.typography.bodySmall,
                        color = IdeFaint,
                    )
                    Spacer(Modifier.height(6.dp))
                    Text(
                        text = stringResource(R.string.about_tagline),
                        style = MaterialTheme.typography.bodyMedium,
                        color = IdeDim,
                        textAlign = TextAlign.Center,
                    )
                }

                // ── Feature list ─────────────────────────────────────────
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 20.dp, vertical = 16.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Text(
                        text = stringResource(R.string.about_features).uppercase(),
                        style = MaterialTheme.typography.labelSmall.copy(
                            fontSize = 10.sp,
                            fontWeight = FontWeight.SemiBold,
                            letterSpacing = 0.8.sp,
                        ),
                        color = IdeAccent.copy(alpha = 0.80f),
                    )
                    ABOUT_FEATURES.forEach { feature ->
                        Row(verticalAlignment = Alignment.Top) {
                            Icon(
                                // §5: thin Outlined check, tinted §3 success green.
                                Icons.Outlined.Check,
                                contentDescription = null,
                                tint = IdeSuccess,
                                modifier = Modifier
                                    .padding(end = 8.dp, top = 2.dp)
                                    .height(16.dp),
                            )
                            Text(
                                text = feature,
                                style = MaterialTheme.typography.bodyMedium,
                                color = IdeDim,
                            )
                        }
                    }
                }

                // ── GitHub link ──────────────────────────────────────────
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clickable {
                            // Open in the system browser; guarded so a device
                            // without a browser does not crash the app.
                            runCatching {
                                context.startActivity(
                                    Intent(Intent.ACTION_VIEW, Uri.parse(GITHUB_URL))
                                )
                            }
                        }
                        .padding(horizontal = 20.dp, vertical = 16.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween,
                ) {
                    Text(
                        text = "github.com/dmytro-yevs/copypaste",
                        style = MaterialTheme.typography.bodyMedium,
                        color = IdeAccent,
                    )
                    Icon(
                        Icons.AutoMirrored.Filled.OpenInNew,
                        contentDescription = stringResource(R.string.about_open_github),
                        tint = IdeAccent,
                        modifier = Modifier.height(16.dp),
                    )
                }
            }
        }
    }
}
