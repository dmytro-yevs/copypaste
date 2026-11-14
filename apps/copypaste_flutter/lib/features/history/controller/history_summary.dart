import 'package:characters/characters.dart';

import '../models/history_models.dart';

/// Matches the server's grapheme-safe 1024-byte list preview contract. A detail
/// read must never turn a lightweight list row into a retained full payload.
HistoryClip historySummary(HistoryClip clip) {
  if (clip.body == null) return clip;
  final preview = StringBuffer();
  var bytes = 0;
  var cut = false;
  for (final cluster in clip.preview.characters) {
    // As on the server, surrounding whitespace is not part of the preview.
    if (bytes == 0 && cluster.trim().isEmpty) continue;
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
    if (bytes > 0 && bytes + clusterBytes > 1024) {
      cut = true;
      break;
    }
    preview.write(cluster);
    bytes += clusterBytes;
  }
  return clip.copyWith(
    preview: preview.toString().trimRight(),
    clearBody: true,
    truncated: clip.truncated || cut,
  );
}

// Retain the row object only when every field supplied to the UI is equal.
// A stable snapshot lets consumers reuse their derived list after refresh.
bool historySummaryEquals(HistoryClip a, HistoryClip b) {
  final af = a.file;
  final bf = b.file;
  final ai = a.image;
  final bi = b.image;
  return a.id == b.id &&
      a.secret == b.secret &&
      a.transient == b.transient &&
      a.contentType == b.contentType &&
      a.preview == b.preview &&
      a.createdAt == b.createdAt &&
      a.pinned == b.pinned &&
      a.kind == b.kind &&
      a.origin == b.origin &&
      a.originDeviceClass == b.originDeviceClass &&
      a.sourceApp == b.sourceApp &&
      a.sourceAppIconId == b.sourceAppIconId &&
      a.truncated == b.truncated &&
      a.lineCount == b.lineCount &&
      a.body == b.body &&
      a.colorRgba == b.colorRgba &&
      a.tooLargeToSync == b.tooLargeToSync &&
      a.annotations == b.annotations &&
      ((af == null && bf == null) ||
          (af != null &&
              bf != null &&
              af.name == bf.name &&
              af.mimeType == bf.mimeType &&
              af.sourceReference == bf.sourceReference &&
              af.sourceAvailable == bf.sourceAvailable &&
              af.sizeBytes == bf.sizeBytes &&
              af.fileCount == bf.fileCount)) &&
      ((ai == null && bi == null) ||
          (ai != null &&
              bi != null &&
              ai.width == bi.width &&
              ai.height == bi.height &&
              ai.sizeBytes == bi.sizeBytes));
}
