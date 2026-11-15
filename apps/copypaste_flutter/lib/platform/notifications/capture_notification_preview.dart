import 'dart:typed_data';

class CaptureNotificationPreview {
  const CaptureNotificationPreview({required this.text, this.imagePng});

  static const _maxRunes = 1000;

  final String text;
  final Uint8List? imagePng;

  static String textPreview(String text) {
    // Normalization maps every rune to one rune except CRLF, which maps two to
    // one, so this prefix yields the same first runes and overflow decision.
    final prefix = String.fromCharCodes(text.runes.take(_maxRunes * 2 + 1));
    final normalized = prefix
        .replaceAll('\r\n', '\n')
        .replaceAll('\r', '\n')
        .replaceAll('\n', '⏎')
        .replaceAll('\t', '⇥');
    final characters = normalized.runes;
    final shortened = String.fromCharCodes(characters.take(_maxRunes));
    return characters.length > _maxRunes ? '$shortened…' : shortened;
  }
}
