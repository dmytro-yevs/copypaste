@file:OptIn(ExperimentalMaterial3Api::class)

package com.copypaste.android

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.copypaste.android.ui.GlassToastHost
import com.copypaste.android.ui.GlassToastKind
import com.copypaste.android.ui.GlassToastState
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.CopyPasteCard
import com.copypaste.android.ui.theme.CopyPasteTopBar
import com.copypaste.android.ui.theme.DarkColors
import com.copypaste.android.ui.theme.LocalAccent
import com.copypaste.android.ui.theme.LocalCpColors
import com.copypaste.android.ui.theme.SecureWindowChrome
import kotlinx.coroutines.launch

class AboutActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            SecureWindowChrome {
                Scaffold { innerPadding ->
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

/**
 * Human version string — version name only, e.g. "0.5.3".
 */
internal fun versionLabel(): String = BuildConfig.VERSION_NAME

/**
 * Version + build number, e.g. "0.5.3 (build 42)" — S11 W4 adds the build
 * number alongside [versionLabel] for support/bug-report identification.
 */
internal fun buildIdentifierLabel(): String =
    "${BuildConfig.VERSION_NAME} (build ${BuildConfig.VERSION_CODE})"

@Composable
fun AboutScreen(
    modifier: Modifier = Modifier,
    showBackButton: Boolean = false,
    onBack: () -> Unit = {},
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val toastState = remember { GlassToastState() }
    val linkFailedMsg = stringResource(R.string.about_link_failed)
    // No isDark composition local is exposed directly; DarkColors/LightColors are the
    // two possible CpColors instances so identity comparison recovers the axis
    // (mirrors the pattern the brand-mark gradient below needs from LocalAccent).
    val isDark = LocalCpColors.current === DarkColors

    // Entrance trigger — flip to true on first composition so the card reveals.
    var entered by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { entered = true }

    Box(modifier = modifier.fillMaxSize()) {
    Scaffold(
        containerColor = Color.Transparent,
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
                .windowInsetsPadding(WindowInsets.navigationBars),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            AnimatedVisibility(
                visible = entered,
                enter = fadeIn(tween(300)),
            ) {
                CopyPasteCard {
                    Column(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalAlignment = Alignment.CenterHorizontally,
                    ) {
                        // Brand mark — mirrors macOS .about__logo (135deg accent->accent-2).
                        Box(
                            modifier = Modifier
                                .size(64.dp)
                                .clip(RoundedCornerShape(18.dp))
                                .background(
                                    Brush.linearGradient(
                                        listOf(LocalAccent.current.base(isDark), LocalAccent.current.variant),
                                    ),
                                ),
                        )
                        Text(text = stringResource(R.string.app_name))
                        Text(text = buildIdentifierLabel())
                        Text(text = stringResource(R.string.about_tagline))
                        Text(text = stringResource(R.string.about_license))
                    }

                    Column(
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        Text(text = stringResource(R.string.about_features).uppercase())
                        ABOUT_FEATURES.forEachIndexed { idx, feature ->
                            val rowAlpha by animateFloatAsState(
                                targetValue = if (entered) 1f else 0f,
                                animationSpec = tween(
                                    durationMillis = 300,
                                    delayMillis = idx * 60,
                                ),
                                label = "featureRow$idx",
                            )
                            Row(
                                verticalAlignment = Alignment.Top,
                                modifier = Modifier.alpha(rowAlpha),
                            ) {
                                Text(text = feature)
                            }
                        }
                    }
                }
            }

            val linkAlpha by animateFloatAsState(
                targetValue = if (entered) 1f else 0f,
                animationSpec = tween(durationMillis = 300, delayMillis = 150),
                label = "aboutLinkAlpha",
            )
            CopyPasteButton(
                onClick = {
                    runCatching {
                        context.startActivity(
                            Intent(Intent.ACTION_VIEW, Uri.parse(GITHUB_URL))
                        )
                    }.onFailure {
                        // No handler for ACTION_VIEW (e.g. no browser installed) — surface a
                        // toast instead of silently swallowing the failure.
                        scope.launch { toastState.show(linkFailedMsg, GlassToastKind.DANGER) }
                    }
                },
                variant = ButtonVariant.SECONDARY,
                modifier = Modifier
                    .fillMaxWidth()
                    .alpha(linkAlpha),
            ) {
                Row {
                    Text(text = "github.com/dmytro-yevs/copypaste")
                }
            }
        }
    }
    GlassToastHost(state = toastState)
    } // end Box
}
