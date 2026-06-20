@file:OptIn(ExperimentalMaterial3Api::class)

package com.copypaste.android

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
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
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.copypaste.android.ui.theme.AuroraDef
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.CopyPasteCard
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.CopyPasteTopBar
import com.copypaste.android.ui.theme.LocalIdeColors
import com.copypaste.android.ui.theme.LocalLiquidTokens
import com.copypaste.android.ui.theme.LocalPalette
import com.copypaste.android.ui.theme.LocalSkin
import com.copypaste.android.ui.theme.Motion
import com.copypaste.android.ui.theme.RadiusChip
import com.copypaste.android.ui.theme.SkinBackground
import com.copypaste.android.ui.theme.auroraCanvas
import com.copypaste.android.ui.theme.glassCanvasBrush
import com.copypaste.android.ui.theme.isDarkTheme
import com.copypaste.android.ui.theme.motionDuration
import com.copypaste.android.ui.theme.paletteAurora
import com.copypaste.android.ui.theme.rememberReducedMotion
import com.copypaste.android.ui.theme.rememberTranslucency
import com.copypaste.android.ui.theme.skinTokens

// ---------------------------------------------------------------------------
// A-C3: tintBlobCanvas — static two-blob backdrop for the VAPOR skin
//
// Unlike auroraCanvas (animated radials covering the full screen), this is a
// STATIC (no animation) soft bicolor blend using the palette's primary glow
// colours anchored at opposite corners.  The opaque base gradient is drawn
// first so the glass blur behind surfaces has real colour to sample
// (PARITY-SPEC §2 requirement — same as auroraCanvas).
//
// [glow] (from SkinTokens.glow) scales blob alpha: Vapor = 0.45,
// giving a more refined, lower-intensity canvas than Classic's 0.62.
// ---------------------------------------------------------------------------

/**
 * Static tint-blob canvas for [SkinBackground.TINT_BLOB] (Vapor skin).
 *
 * Draws an opaque base gradient from [auroraDef]'s bg ramp, then overlays
 * two large soft blobs at opposite corners ([auroraDef.glowA] top-left,
 * [auroraDef.glowB] bottom-right) and a small centre accent from
 * [auroraDef.overlayAccent]. All blob alphas are scaled by [glow] so the
 * intensity matches [SkinTokens.glow].
 *
 * Apply to the Scaffold modifier; host must use `containerColor = Transparent`.
 */
private fun Modifier.tintBlobCanvas(
    dark: Boolean,
    auroraDef: AuroraDef,
    glow: Float,
): Modifier = this.drawBehind {
    // Opaque base — glass blur needs real colour behind surfaces (PARITY-SPEC §2).
    drawRect(glassCanvasBrush(dark, auroraDef))

    val diag = kotlin.math.hypot(size.width, size.height)

    // Primary blob — top-left corner, large radius, palette glowA.
    val blobA = auroraDef.glowA.copy(alpha = (auroraDef.glowA.alpha * glow * 1.4f).coerceIn(0f, 1f))
    drawRect(
        brush = Brush.radialGradient(
            colorStops = arrayOf(0.0f to blobA, 0.55f to Color.Transparent),
            center = Offset(size.width * 0.08f, size.height * 0.10f),
            radius = diag * 0.90f,
        ),
    )

    // Secondary blob — bottom-right corner, slightly smaller, palette glowB.
    val blobB = auroraDef.glowB.copy(alpha = (auroraDef.glowB.alpha * glow * 1.4f).coerceIn(0f, 1f))
    drawRect(
        brush = Brush.radialGradient(
            colorStops = arrayOf(0.0f to blobB, 0.55f to Color.Transparent),
            center = Offset(size.width * 0.92f, size.height * 0.88f),
            radius = diag * 0.80f,
        ),
    )

    // Centre accent — subtle overlayAccent warms the middle of the canvas.
    val centre = auroraDef.overlayAccent.copy(
        alpha = (auroraDef.overlayAccent.alpha * glow).coerceIn(0f, 1f),
    )
    drawRect(
        brush = Brush.radialGradient(
            colorStops = arrayOf(0.0f to centre, 0.65f to Color.Transparent),
            center = Offset(size.width * 0.50f, size.height * 0.42f),
            radius = diag * 0.30f,
        ),
    )
}

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

                // A-C3: gate the background canvas by tok.background so each skin
                // gets its intended backdrop — AURORA for CLASSIC, FLAT (no canvas)
                // for QUIET, TINT_BLOB (static accent blob) for VAPOR.
                val tok = skinTokens(LocalSkin.current)
                val showAurora = translucent && tok.background == SkinBackground.AURORA
                val showTintBlob = translucent && tok.background == SkinBackground.TINT_BLOB
                val showCanvas = showAurora || showTintBlob

                val canvasModifier = when {
                    showAurora   -> Modifier.auroraCanvas(dark, paletteAurora(LocalPalette.current))
                    showTintBlob -> Modifier.tintBlobCanvas(dark, paletteAurora(LocalPalette.current), tok.glow)
                    else         -> Modifier
                }

                Scaffold(
                    // Container is transparent whenever a canvas backdrop is active so
                    // the aurora or tint-blob gradient shows through. CLASSIC is unchanged.
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

/** Human version string, e.g. "0.5.3 (build 8)". */
private fun versionLabel(): String =
    "${BuildConfig.VERSION_NAME} (build ${BuildConfig.VERSION_CODE})"

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
    val lt = LocalLiquidTokens.current
    val dark = isDarkTheme()
    val translucent = rememberTranslucency()
    val reduced = rememberReducedMotion()
    val slowDur = motionDuration(Motion.Slow)
    val baseDur = motionDuration(Motion.Base)

    // Entrance trigger — flip to true on first composition so the card reveals.
    var entered by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { entered = true }

    // Card slide-up + fade entrance — 0ms when reduced motion is active.
    val cardAlpha by animateFloatAsState(
        targetValue = if (entered) 1f else 0f,
        animationSpec = tween(if (reduced) 0 else slowDur),
        label = "aboutCardAlpha",
    )

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
            CopyPasteCard(
                modifier = Modifier.alpha(cardAlpha),
            ) {
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
                        color = lt.accent2.copy(alpha = 0.70f),
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
