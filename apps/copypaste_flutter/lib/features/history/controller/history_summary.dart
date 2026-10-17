import 'package:characters/characters.dart';

import '../models/history_models.dart';

/// Matches the server's grapheme-safe 1024-byte list preview contract. A detail
/// read must never turn a lightweight list row into a retained full payload.
HistoryClip historySummary(HistoryClip clip) {
  if (clip.body == null) return clip;
  final preview = StringBuffer();
  var bytes = 0;
  for (final cluster in clip.preview.characters) {
    var clusterBytes = 0;
    for (final rune in cluster.runes) {
      clusterBytes += rune <= 0x7f
          ? 1
          : rune <= 0x7ff
          ? 2
          : rune <= 0xffff
          ? 3
          : 4;
    }
    // As on the server, preserve an indivisible first grapheme.
    if (bytes > 0 && bytes + clusterBytes > 1024) break;
    preview.write(cluster);
    bytes += clusterBytes;
  }
  final bounded = preview.toString();
  return clip.copyWith(
    preview: bounded,
    clearBody: true,
    truncated: clip.truncated || bounded.length < clip.preview.length,
  );
}
