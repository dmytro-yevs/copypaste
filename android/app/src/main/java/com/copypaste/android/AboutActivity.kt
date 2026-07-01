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
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
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
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.CopyPasteCard
import com.copypaste.android.ui.theme.CopyPasteTopBar
import com.copypaste.android.ui.theme.SecureWindowChrome

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

@Composable
fun AboutScreen(
    modifier: Modifier = Modifier,
    showBackButton: Boolean = false,
    onBack: () -> Unit = {},
) {
    val context = LocalContext.current

    // Entrance trigger — flip to true on first composition so the card reveals.
    var entered by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { entered = true }

    Scaffold(
        modifier = modifier,
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
                        Text(text = stringResource(R.string.app_name))
                        Text(text = versionLabel())
                        Text(text = stringResource(R.string.about_tagline))
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
}
