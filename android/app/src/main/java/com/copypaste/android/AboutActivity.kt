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
import androidx.compose.animation.slideInVertically
import com.copypaste.android.ui.theme.EaseOutExpo
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.windowInsetsPadding
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
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.CopyPasteCard
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.CopyPasteTopBar
import com.copypaste.android.ui.theme.LocalAccent
import com.copypaste.android.ui.theme.LocalIdeColors
import com.copypaste.android.ui.theme.Motion
import com.copypaste.android.ui.theme.RadiusChip
import com.copypaste.android.ui.theme.isDarkTheme
import com.copypaste.android.ui.theme.motionDuration
import com.copypaste.android.ui.theme.rememberReducedMotion
import com.copypaste.android.ui.theme.rememberTranslucency
import com.copypaste.android.ui.theme.screenCanvas

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
                val c = LocalIdeColors.current
                val dark = isDarkTheme()
                val translucent = rememberTranslucency()

                // Calm screen backdrop (STYLEGUIDE §6 — no aurora). Frosted only when translucent.
                val showCanvas = translucent
                val canvasModifier = if (showCanvas) Modifier.screenCanvas(dark) else Modifier

                Scaffold(
                    containerColor = if (showCanvas) Color.Transparent else c.bg,
                    modifier = canvasModifier,
                ) { innerPadding ->
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
 *
 * CopyPaste-bdac.77: macOS AboutView shows only the version name (from
 * Tauri getVersion() / app_version IPC, which expose no build-number
 * equivalent). Android previously showed "VERSION_NAME (build VERSION_CODE)".
 * Aligned to macOS format: display VERSION_NAME only so both platforms show
 * the same string and users report a consistent version number.
 *
 * VERSION_CODE is used by the Play Store / package manager for ordering; it
 * is not displayed in the About screen on either platform.
 */
internal fun versionLabel(): String = BuildConfig.VERSION_NAME

/**
 * About content, hostable inside [MainActivity]'s nav shell or the standalone
 * [AboutActivity]. Styled with the shared IDE-theme components
 * (CopyPasteTopBar / CopyPasteCard / IDE color tokens) so it reads as a sibling
 * of the History / Pair / Settings screens.
 *
 * Premium entrance: the identity card slides up and fades in; feature rows stagger
 * in with a slight delay each. All animation is zeroed when reduced motion is active.
 */
@Composable
fun AboutScreen(
    modifier: Modifier = Modifier,
    showBackButton: Boolean = false,
    onBack: () -> Unit = {},
) {
    val context = LocalContext.current
    val c = LocalIdeColors.current
    val accentVariant = LocalAccent.current.variant
    val reduced = rememberReducedMotion()
    val slowDur = motionDuration(Motion.Slow)
    val baseDur = motionDuration(Motion.Base)

    // Entrance trigger — flip to true on first composition so the card reveals.
    var entered by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { entered = true }

    // Card Y-slide + fade entrance — matches GlassToast pattern (§8 EaseOutExpo, Motion.Slow).
    // AnimatedVisibility gives us a slideInVertically so the card rises 1/12 of its own
    // height and fades in simultaneously — more premium than alpha-only.
    // (The old animateFloatAsState cardAlpha approach is replaced here; cardAlpha is retained
    //  for backward compat but now unused — the AnimatedVisibility wrapper below takes over.)

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
                .windowInsetsPadding(WindowInsets.navigationBars)
                .padding(horizontal = 20.dp, vertical = 12.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            // ── Identity + Features card ──────────────────────────────────────
            // Single glass card contains both blocks so they read as one premium unit.
            // §8 entrance: slideInVertically (1/12 height) + fadeIn, EaseOutExpo, Motion.Slow.
            // Mirrors GlassToast enter transition for a consistent premium settling feel.
            AnimatedVisibility(
                visible = entered,
                enter = if (reduced) fadeIn(tween(0))
                else slideInVertically(
                    animationSpec = tween(slowDur, easing = EaseOutExpo),
                    initialOffsetY = { it / 12 },
                ) + fadeIn(tween(slowDur, easing = EaseOutExpo)),
            ) {
            CopyPasteCard {
                // ── Identity ─────────────────────────────────────────────────
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 24.dp, vertical = 28.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    // App name — headline display (largest type)
                    Text(
                        text = stringResource(R.string.app_name),
                        style = MaterialTheme.typography.displayLarge,
                        color = c.text,
                        textAlign = TextAlign.Center,
                    )
                    Spacer(Modifier.height(4.dp))
                    // Version badge — small glass chip pill
                    Box(
                        modifier = Modifier
                            .background(c.accentDim, RadiusChip)
                            .border(0.5.dp, c.accent.copy(alpha = 0.35f), RadiusChip)
                            .padding(horizontal = 10.dp, vertical = 3.dp),
                    ) {
                        Text(
                            text = versionLabel(),
                            style = MaterialTheme.typography.labelSmall.copy(
                                fontFamily = com.copypaste.android.ui.theme.MonoFontFamily,
                                fontSize = 11.sp,
                            ),
                            color = c.accent,
                        )
                    }
                    Spacer(Modifier.height(10.dp))
                    Text(
                        text = stringResource(R.string.about_tagline),
                        style = MaterialTheme.typography.bodyMedium,
                        color = c.dim,
                        textAlign = TextAlign.Center,
                    )
                }

                // Hairline divider between identity and feature list
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(0.5.dp)
                        .background(c.divider),
                )

                // ── Feature list ─────────────────────────────────────────────
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 20.dp, vertical = 18.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    // Section label — muted uppercase Apple-style header
                    Text(
                        text = stringResource(R.string.about_features).uppercase(),
                        style = MaterialTheme.typography.labelSmall.copy(
                            fontSize = 10.sp,
                            fontWeight = FontWeight.SemiBold,
                            letterSpacing = 0.8.sp,
                        ),
                        color = accentVariant.copy(alpha = 0.70f),
                    )
                    ABOUT_FEATURES.forEachIndexed { idx, feature ->
                        // Each feature row fades in with a small stagger delay.
                        val rowAlpha by animateFloatAsState(
                            targetValue = if (entered) 1f else 0f,
                            animationSpec = tween(
                                durationMillis = if (reduced) 0 else baseDur,
                                delayMillis = if (reduced) 0 else (idx * 60),
                            ),
                            label = "featureRow$idx",
                        )
                        Row(
                            verticalAlignment = Alignment.Top,
                            horizontalArrangement = Arrangement.spacedBy(10.dp),
                            modifier = Modifier.alpha(rowAlpha),
                        ) {
                            // Success-tinted check icon — maps to c.success
                            Icon(
                                Icons.Outlined.Check,
                                contentDescription = null,
                                tint = c.success,
                                modifier = Modifier
                                    .padding(top = 1.dp)
                                    .size(16.dp),
                            )
                            Text(
                                text = feature,
                                style = MaterialTheme.typography.bodyMedium,
                                color = c.dim,
                            )
                        }
                    }
                }
            }
            } // end AnimatedVisibility (card entrance)

            // ── GitHub link chip ──────────────────────────────────────────────
            // Tasteful standalone chip below the card; fades in after card.
            val linkAlpha by animateFloatAsState(
                targetValue = if (entered) 1f else 0f,
                animationSpec = tween(
                    durationMillis = if (reduced) 0 else slowDur,
                    delayMillis = if (reduced) 0 else (baseDur / 2),
                ),
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
                Row(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(
                        text = "github.com/dmytro-yevs/copypaste",
                        style = MaterialTheme.typography.bodyMedium,
                        color = c.accent,
                    )
                    Icon(
                        Icons.AutoMirrored.Filled.OpenInNew,
                        contentDescription = stringResource(R.string.about_open_github),
                        tint = c.accent,
                        modifier = Modifier.size(16.dp),
                    )
                }
            }

            Spacer(Modifier.height(8.dp))
        }
    }
}
