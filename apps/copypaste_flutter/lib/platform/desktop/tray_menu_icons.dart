import 'dart:convert';
import 'dart:ui' as ui;

import 'package:flutter/widgets.dart';
import 'package:tray_manager/tray_manager.dart' as native;

import '../../app/theme/app_tokens.dart';

/// Renders the shared icon font for native menus, which require image data.
Future<native.Image> createTrayMenuIcon(IconData icon) async {
  const scale = 2.0;
  const size = AppIconSize.sm;
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
  final recorder = ui.PictureRecorder();
  final canvas = Canvas(recorder)..scale(scale);
  painter.paint(
    canvas,
    Offset((size - painter.width) / 2, (size - painter.height) / 2),
  );
  painter.dispose();
  final picture = recorder.endRecording();
  final ui.Image raster;
  try {
    raster = await picture.toImage(
      (size * scale).round(),
      (size * scale).round(),
    );
  } finally {
    picture.dispose();
  }
  try {
    final bytes = await raster.toByteData(format: ui.ImageByteFormat.png);
    if (bytes == null) {
      throw StateError('Unable to render a system tray menu icon.');
    }
    final image = native.Image.fromBase64(
      'data:image/png;base64,${base64Encode(bytes.buffer.asUint8List(bytes.offsetInBytes, bytes.lengthInBytes))}',
    );
    if (image == null) {
      throw StateError('Unable to create a system tray menu icon.');
    }
    return image;
  } finally {
    raster.dispose();
  }
}
