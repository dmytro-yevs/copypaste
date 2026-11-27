import 'dart:convert';
import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter/widgets.dart';
import 'package:image/image.dart' as img;
import 'package:tray_manager/tray_manager.dart' as native;

import '../../app/theme/app_tokens.dart';

/// Renders the shared icon font for native menus, which require image data.
///
/// All [icons] share one picture and one readback. Every offscreen image is a
/// render pass of its own, and a burst of passes makes the GPU driver hold
/// pooled memory for each of them.
Future<Map<IconData, native.Image>> createTrayMenuIcons(
  Iterable<IconData> icons,
) async {
  const scale = 2.0;
  const size = AppIconSize.sm;
  final glyphs = icons.toList(growable: false);
  if (glyphs.isEmpty) return const {};
  final edge = (size * scale).round();
  final recorder = ui.PictureRecorder();
  final canvas = Canvas(recorder)..scale(scale);
  for (final (index, icon) in glyphs.indexed) {
    final painter = TextPainter(
      text: TextSpan(
        text: String.fromCharCode(icon.codePoint),
        style: TextStyle(
          fontFamily: icon.fontFamily,
          package: icon.fontPackage,
          fontSize: size,
          color: const Color(0xff000000),
          height: 1,
        ),
      ),
      textDirection: TextDirection.ltr,
    )..layout();
    painter.paint(
      canvas,
      Offset(
        index * size + (size - painter.width) / 2,
        (size - painter.height) / 2,
      ),
    );
    painter.dispose();
  }
  final picture = recorder.endRecording();
  final ui.Image raster;
  try {
    raster = await picture.toImage(edge * glyphs.length, edge);
  } finally {
    picture.dispose();
  }
  final ByteData? pixels;
  try {
    pixels = await raster.toByteData(
      format: ui.ImageByteFormat.rawStraightRgba,
    );
  } finally {
    raster.dispose();
  }
  if (pixels == null) {
    throw StateError('Unable to render the system tray menu icons.');
  }
  final sheet = img.Image.fromBytes(
    width: edge * glyphs.length,
    height: edge,
    bytes: pixels.buffer,
    bytesOffset: pixels.offsetInBytes,
    numChannels: 4,
  );
  final images = <IconData, native.Image>{};
  for (final (index, icon) in glyphs.indexed) {
    final glyph = img.copyCrop(
      sheet,
      x: index * edge,
      y: 0,
      width: edge,
      height: edge,
    );
    final image = native.Image.fromBase64(
      'data:image/png;base64,${base64Encode(img.encodePng(glyph))}',
    );
    if (image == null) {
      for (final created in images.values) {
        created.dispose();
      }
      throw StateError('Unable to create a system tray menu icon.');
    }
    images[icon] = image;
  }
  return images;
}
