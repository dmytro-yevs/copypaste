import 'dart:convert';

final _unsafeCharacters = RegExp(r'[\x00-\x1F\x7F<>:"|?*]');
const _maxExtensionLength = 16;

/// Returns a file-system-safe basename that keeps Unicode, like the daemon.
///
/// Only path separators, control characters, and Windows-reserved characters
/// are replaced. Names longer than [maxBytes] UTF-8 bytes are shortened before
/// a short extension so the file type survives. Android's clipboard writer
/// implements the same rule; both are checked against one shared fixture.
String safeFileName(
  String? value, {
  int maxBytes = 255,
  String fallback = 'file',
}) {
  var name = (value ?? '')
      .split(RegExp(r'[/\\]'))
      .last
      .replaceAll(_unsafeCharacters, '_')
      .trim();
  if (name.isEmpty || name == '.' || name == '..') return fallback;
  if (utf8.encode(name).length <= maxBytes) return name;
  final dot = name.lastIndexOf('.');
  final extension = dot > 0 && name.length - dot <= _maxExtensionLength
      ? name.substring(dot)
      : '';
  var budget = maxBytes - utf8.encode(extension).length;
  final stem = StringBuffer();
  for (final rune in name.substring(0, name.length - extension.length).runes) {
    final character = String.fromCharCode(rune);
    budget -= utf8.encode(character).length;
    if (budget < 0) break;
    stem.write(character);
  }
  name = '${stem.toString().trimRight()}$extension';
  return name.isEmpty ? fallback : name;
}
