import 'package:flutter/painting.dart';

/// Fits a path to the measured row width while keeping both ends visible.
String middleEllipsis(
  String value, {
  required double maxWidth,
  required TextStyle style,
  required TextDirection direction,
  TextScaler textScaler = TextScaler.noScaling,
}) {
  if (!maxWidth.isFinite || maxWidth <= 0) return '';
  bool fits(String text) {
    final painter = TextPainter(
      text: TextSpan(text: text, style: style),
      textDirection: direction,
      maxLines: 1,
      textScaler: textScaler,
    )..layout();
    return painter.width <= maxWidth;
  }

  if (fits(value)) return value;
  const ellipsis = '…';
  if (!fits(ellipsis)) return '';
  final runes = value.runes.toList(growable: false);
  var low = 0;
  var high = runes.length;
  while (low < high) {
    final count = (low + high + 1) ~/ 2;
    final first = (count + 2) ~/ 3;
    final last = count - first;
    final candidate =
        '${String.fromCharCodes(runes.take(first))}$ellipsis${String.fromCharCodes(runes.skip(runes.length - last))}';
    if (fits(candidate)) {
      low = count;
    } else {
      high = count - 1;
    }
  }
  final first = (low + 2) ~/ 3;
  final last = low - first;
  return '${String.fromCharCodes(runes.take(first))}$ellipsis${String.fromCharCodes(runes.skip(runes.length - last))}';
}
