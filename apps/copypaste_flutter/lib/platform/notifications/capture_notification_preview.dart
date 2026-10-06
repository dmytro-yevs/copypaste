import 'dart:typed_data';

class CaptureNotificationPreview {
  const CaptureNotificationPreview({required this.text, this.imagePng});

  final String text;
  final Uint8List? imagePng;

  static String textPreview(String text) {
    final normalized = text
        .replaceAll('\r\n', '\n')
        .replaceAll('\r', '\n')
        .replaceAll('\n', '⏎')
        .replaceAll('\t', '⇥');
    final characters = normalized.runes;
    final shortened = String.fromCharCodes(characters.take(1000));
    return characters.length > 1000 ? '$shortened…' : shortened;
  }
}
