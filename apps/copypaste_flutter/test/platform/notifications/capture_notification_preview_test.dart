import 'dart:math';

import 'package:copypaste_flutter/platform/notifications/capture_notification_preview.dart';
import 'package:flutter_test/flutter_test.dart';

String _fullLengthPreview(String text) {
  final normalized = text
      .replaceAll('\r\n', '\n')
      .replaceAll('\r', '\n')
      .replaceAll('\n', '⏎')
      .replaceAll('\t', '⇥');
  final characters = normalized.runes;
  final shortened = String.fromCharCodes(characters.take(1000));
  return characters.length > 1000 ? '$shortened…' : shortened;
}

void main() {
  test('bounded text previews match full-length normalization', () {
    final random = Random(7);
    const parts = ['a', '\r\n', '\r', '\n', '\t', '😀'];
    final inputs = [
      '\r\n' * 1000,
      '${'\r\n' * 1000}a',
      '\r\n' * 1001,
      '${'a' * 999}\r\nb',
      '${'a' * 2000}\r\n',
      '😀' * 1001,
      for (var sample = 0; sample < 200; sample++)
        [
          for (var part = random.nextInt(2600); part > 0; part--)
            parts[random.nextInt(parts.length)],
        ].join(),
    ];
    for (final input in inputs) {
      expect(
        CaptureNotificationPreview.textPreview(input),
        _fullLengthPreview(input),
      );
    }
  });
}
