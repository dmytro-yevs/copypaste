import 'dart:convert';
import 'dart:io';

import 'package:copypaste_flutter/platform/files/file_names.dart';
import 'package:copypaste_flutter/platform/notifications/capture_notification_preview.dart';
import 'package:flutter_test/flutter_test.dart';

/// The Android host checks the same fixture in `NativeTextVectorsTest`.
void main() {
  final vectors =
      jsonDecode(
            File('test/fixtures/native_text_vectors.json').readAsStringSync(),
          )
          as Map<String, Object?>;

  test('file names match the shared native vectors', () {
    for (final vector in (vectors['safeFileName'] as List).cast<Map>()) {
      expect(
        safeFileName(
          vector['input'] as String,
          maxBytes: vector['maxBytes'] as int,
        ),
        vector['expected'],
        reason: vector['input'] as String,
      );
    }
  });

  test('capture previews match the shared native vectors', () {
    for (final vector
        in (vectors['captureNotificationPreview'] as List).cast<Map>()) {
      expect(
        CaptureNotificationPreview.textPreview(vector['input'] as String),
        vector['expected'],
      );
    }
  });
}
