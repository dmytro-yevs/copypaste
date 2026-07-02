#!/usr/bin/env node
// generate-lucide-icons.mjs — vendors a curated Lucide icon subset as Kotlin
// ImageVector.Builder source, one top-level `val` per icon (one file per
// icon, R8-shakeable — matches the pattern androidx.compose.material's
// generated material-icons-extended pack uses).
//
// Invoked by scripts/generate-lucide-icons.sh, which pins the Lucide tag/SHA.
// This script owns the SVG-shape → Compose PathBuilder DSL conversion (S0.3
// deviation: DevSrSouza/svg-to-compose has no published Maven/JitPack
// artifact reachable from this toolchain — see design.md "Resolved
// decisions" and the S2 bd notes for the recorded rationale). It mirrors
// svg-to-compose's job (SVG -> ImageVector Kotlin source) using only the
// stable public androidx.compose.ui.graphics.vector.PathBuilder DSL
// (moveTo/lineTo/horizontalLineTo/verticalLineTo/curveTo/reflectiveCurveTo/
// arcTo/close + their *Relative variants), which is a 1:1 mirror of the SVG
// path command grammar this script parses.
//
// Usage: node scripts/generate-lucide-icons.mjs <lucideSha> <outDir>

import { writeFileSync, mkdirSync } from "node:fs";
import { join } from "node:path";
import { execFileSync } from "node:child_process";

const [, , LUCIDE_SHA, OUT_DIR] = process.argv;
if (!LUCIDE_SHA || !OUT_DIR) {
  console.error("usage: generate-lucide-icons.mjs <lucideSha> <outDir>");
  process.exit(1);
}

// role/consumer -> [Kotlin property name, Lucide icon file name]
// NOTE: "shield-x" (STYLEGUIDE / cross-platform-parity.md icon table's
// canonical name for the revoke action) does not exist in Lucide at the
// pinned tag; substituted with "shield-alert" (closest semantic match —
// "revoke trust" warning). Recorded as a deviation in bd notes.
const ICONS = [
  ["ArrowLeft", "arrow-left"],
  ["History", "history"],
  ["MonitorSmartphone", "monitor-smartphone"],
  ["Settings2", "settings-2"],
  ["AlignLeft", "align-left"],
  ["Link", "link"],
  ["Mail", "mail"],
  ["Phone", "phone"],
  ["Code", "code"],
  ["Braces", "braces"],
  ["Hash", "hash"],
  ["FileIcon", "file"],
  ["Folder", "folder"],
  ["Lock", "lock"],
  ["CheckCircle", "check-circle"],
  ["AlertTriangle", "alert-triangle"],
  ["AlertCircle", "alert-circle"],
  ["Info", "info"],
  ["Pin", "pin"],
  ["Trash2", "trash-2"],
  ["Copy", "copy"],
  ["Eye", "eye"],
  ["Unlink", "unlink"],
  ["ShieldAlert", "shield-alert"],
  ["Inbox", "inbox"],
  ["FileText", "file-text"],
  ["Clock", "clock"],
  ["CircleDashed", "circle-dashed"],
  // CopyPaste-myh8.8 (S8, hand-appended when QrCode.kt was vendored — added
  // here now for regeneration parity, per that file's header note).
  ["QrCode", "qr-code"],
  // S6/S7 fix round (CopyPaste-myh8.6/.7): 4 new glyphs migrating the last
  // material-icons-extended call sites in PreviewChrome.kt/PreviewActionRow.kt.
  ["X", "x"],
  ["ExternalLink", "external-link"],
  ["Download", "download"],
  ["Bookmark", "bookmark"],
];

const RAW_BASE = `https://raw.githubusercontent.com/lucide-icons/lucide/${LUCIDE_SHA}/icons`;

function fetchSvg(name) {
  const url = `${RAW_BASE}/${name}.svg`;
  const res = spawnCurl(url);
  if (!res || res.startsWith("404")) {
    throw new Error(`failed to fetch ${url}: ${res}`);
  }
  return res;
}

function spawnCurl(url) {
  return execFileSync("curl", ["-sS", "-m", "15", url], { encoding: "utf8" });
}

// --- SVG element -> path `d` string normalization ---------------------------

function attr(tag, name) {
  const m = tag.match(new RegExp(`${name}="([^"]*)"`));
  return m ? m[1] : undefined;
}

function circleToD(tag) {
  const cx = parseFloat(attr(tag, "cx"));
  const cy = parseFloat(attr(tag, "cy"));
  const r = parseFloat(attr(tag, "r"));
  return `M ${cx - r} ${cy} A ${r} ${r} 0 1 0 ${cx + r} ${cy} A ${r} ${r} 0 1 0 ${cx - r} ${cy}`;
}

function lineToD(tag) {
  const x1 = attr(tag, "x1");
  const y1 = attr(tag, "y1");
  const x2 = attr(tag, "x2");
  const y2 = attr(tag, "y2");
  return `M ${x1} ${y1} L ${x2} ${y2}`;
}

function rectToD(tag) {
  const x = parseFloat(attr(tag, "x") ?? "0");
  const y = parseFloat(attr(tag, "y") ?? "0");
  const w = parseFloat(attr(tag, "width"));
  const h = parseFloat(attr(tag, "height"));
  const rx = parseFloat(attr(tag, "rx") ?? attr(tag, "ry") ?? "0") || 0;
  if (!rx) {
    return `M ${x} ${y} H ${x + w} V ${y + h} H ${x} Z`;
  }
  return (
    `M ${x + rx} ${y} H ${x + w - rx} A ${rx} ${rx} 0 0 1 ${x + w} ${y + rx} ` +
    `V ${y + h - rx} A ${rx} ${rx} 0 0 1 ${x + w - rx} ${y + h} ` +
    `H ${x + rx} A ${rx} ${rx} 0 0 1 ${x} ${y + h - rx} ` +
    `V ${y + rx} A ${rx} ${rx} 0 0 1 ${x + rx} ${y} Z`
  );
}

function pointsToD(tag, close) {
  const pts = attr(tag, "points").trim().split(/\s+/).map(Number);
  let d = `M ${pts[0]} ${pts[1]}`;
  for (let i = 2; i < pts.length; i += 2) {
    d += ` L ${pts[i]} ${pts[i + 1]}`;
  }
  if (close) d += " Z";
  return d;
}

/** Extracts every drawable shape element from an SVG body as a list of `d` path strings. */
function extractPathData(svg) {
  const ds = [];
  const tagRe = /<(path|circle|line|rect|polyline|polygon)\b[^>]*\/?>/g;
  let m;
  while ((m = tagRe.exec(svg))) {
    const tag = m[0];
    const kind = m[1];
    if (kind === "path") ds.push(attr(tag, "d"));
    else if (kind === "circle") ds.push(circleToD(tag));
    else if (kind === "line") ds.push(lineToD(tag));
    else if (kind === "rect") ds.push(rectToD(tag));
    else if (kind === "polyline") ds.push(pointsToD(tag, false));
    else if (kind === "polygon") ds.push(pointsToD(tag, true));
  }
  return ds;
}

// --- SVG path `d` grammar -> Compose PathBuilder DSL -------------------------

const ARG_COUNTS = { M: 2, L: 2, H: 1, V: 1, C: 6, S: 4, A: 7, Z: 0 };

function tokenizeD(d) {
  const tokens = [];
  const re = /([MLHVCSAZmlhvcsaz])|(-?\d*\.?\d+(?:e-?\d+)?)/g;
  let m;
  while ((m = re.exec(d))) {
    if (m[1]) tokens.push({ cmd: m[1] });
    else tokens.push({ num: parseFloat(m[2]) });
  }
  return tokens;
}

/** Emits one Compose PathBuilder DSL line per SVG path command (relative variants map 1:1). */
function dToKotlinCalls(d) {
  const tokens = tokenizeD(d);
  const calls = [];
  let i = 0;
  let cmd = null;
  while (i < tokens.length) {
    if (tokens[i].cmd) {
      cmd = tokens[i].cmd;
      i++;
    }
    const upper = cmd.toUpperCase();
    const relative = cmd !== upper;
    const nArgs = ARG_COUNTS[upper];
    if (nArgs === 0) {
      calls.push("close()");
      // Z has no repeat group; a further coordinate group after Z would be a new M.
      continue;
    }
    const args = [];
    for (let k = 0; k < nArgs; k++) {
      args.push(tokens[i + k].num);
    }
    i += nArgs;
    calls.push(emitCall(upper, relative, args));
    // Per SVG grammar, M followed by extra coordinate pairs implies L (l).
    if (upper === "M") cmd = relative ? "l" : "L";
  }
  return calls;
}

function f(n) {
  return `${n}f`;
}

function emitCall(upper, relative, args) {
  const suffix = relative ? "Relative" : "";
  switch (upper) {
    case "M":
      return `moveTo${suffix}(${f(args[0])}, ${f(args[1])})`;
    case "L":
      return `lineTo${suffix}(${f(args[0])}, ${f(args[1])})`;
    case "H":
      return `horizontalLineTo${suffix}(${f(args[0])})`;
    case "V":
      return `verticalLineTo${suffix}(${f(args[0])})`;
    case "C":
      return `curveTo${suffix}(${args.map(f).join(", ")})`;
    case "S":
      return `reflectiveCurveTo${suffix}(${args.map(f).join(", ")})`;
    case "A": {
      const [rx, ry, rot, largeArc, sweep, x, y] = args;
      return `arcTo${suffix}(${f(rx)}, ${f(ry)}, ${f(rot)}, ${largeArc === 1}, ${sweep === 1}, ${f(x)}, ${f(y)})`;
    }
    default:
      throw new Error(`unsupported command ${upper}`);
  }
}

const HEADER = `// GENERATED FILE — DO NOT EDIT BY HAND.
// Produced by scripts/generate-lucide-icons.sh from lucide-icons/lucide
// (ISC license) at the pinned SHA recorded in that script's header.
// Regenerate with: ./scripts/generate-lucide-icons.sh
package com.copypaste.android.ui.theme.icons

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.path
import androidx.compose.ui.unit.dp

// Lucide glyphs render with fill=none / stroke=currentColor; the tint below is
// a build-time placeholder — every call site supplies the real tint via
// Icon(tint = ...), matching the fill=none/stroke=currentColor SVG contract.
private val NoFill = SolidColor(Color.Transparent)
private val PlaceholderStroke = SolidColor(Color.Black)
`;

function kotlinFileFor(propName, ds) {
  const pathBlocks = ds
    .map(
      (d) => `            path(
                fill = NoFill,
                stroke = PlaceholderStroke,
                strokeLineWidth = 2f,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
                pathFillType = PathFillType.NonZero,
            ) {
${dToKotlinCalls(d)
  .map((c) => `                ${c}`)
  .join("\n")}
            }`,
    )
    .join("\n");
  return `${HEADER}
/** Lucide "${propName}" glyph — 24x24 viewBox, stroke-width 2, round caps/joins. */
val ${propName}: ImageVector by lazy {
    ImageVector.Builder(
        name = "Lucide.${propName}",
        defaultWidth = 24.dp,
        defaultHeight = 24.dp,
        viewportWidth = 24f,
        viewportHeight = 24f,
    ).apply {
${pathBlocks}
    }.build()
}
`;
}

mkdirSync(OUT_DIR, { recursive: true });
for (const [propName, iconName] of ICONS) {
  const svg = fetchSvg(iconName);
  const ds = extractPathData(svg);
  if (ds.length === 0) throw new Error(`no shapes extracted for ${iconName}`);
  const content = kotlinFileFor(propName, ds);
  writeFileSync(join(OUT_DIR, `${propName}.kt`), content, "utf8");
  console.log(`wrote ${propName}.kt (${ds.length} path segments, source icons/${iconName}.svg)`);
}
