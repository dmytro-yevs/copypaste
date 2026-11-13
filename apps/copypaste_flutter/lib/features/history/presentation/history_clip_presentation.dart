import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../models/history_models.dart';

/// The canonical visual mapping for every clipboard content kind.
abstract final class HistoryClipPresentation {
  static IconData icon(HistoryClipKind kind) => switch (kind) {
    HistoryClipKind.text => LucideIcons.type,
    HistoryClipKind.link => LucideIcons.link,
    HistoryClipKind.email => LucideIcons.mail,
    HistoryClipKind.color => LucideIcons.palette,
    HistoryClipKind.phone => LucideIcons.phone,
    HistoryClipKind.code => LucideIcons.code,
    HistoryClipKind.json => LucideIcons.braces,
    HistoryClipKind.path => LucideIcons.route,
    HistoryClipKind.credentials => LucideIcons.keyRound,
    HistoryClipKind.image => LucideIcons.image,
    HistoryClipKind.file => LucideIcons.file,
    HistoryClipKind.other => LucideIcons.box,
  };

  /// The marker of a clip CopyPaste created, shown next to its source app.
  static String? provenance(
    AppLocalizations strings,
    HistoryProvenance? provenance,
  ) => switch (provenance) {
    HistoryProvenance.manual => strings.historyProvenanceManual,
    HistoryProvenance.edited => strings.historyProvenanceEdited,
    HistoryProvenance.transformed => strings.historyProvenanceTransformed,
    HistoryProvenance.aiTool => strings.historyProvenanceAiTool,
    null => null,
  };

  /// The most displayed lines a History row gives to a text preview.
  static const previewMaxLines = 4;

  /// The clip's line count when its row certainly shows fewer lines, else null.
  ///
  /// Lines are the line-feed separated lines of the complete text: the
  /// runtime's count, else the preview's only when it is the whole text. Each
  /// takes at least one displayed line, so a count above [previewMaxLines]
  /// always means a cut row; wrapped long lines and hidden clips stay
  /// unreported.
  static int? cutLineCount(HistoryClip clip) {
    if (clip.masked || !clip.contentKind.isTextual) return null;
    final lines = clip.lineCount ?? _previewLineCount(clip);
    return lines != null && lines > previewMaxLines ? lines : null;
  }

  static int? _previewLineCount(HistoryClip clip) {
    if (clip.truncated) return null;
    final text = clip.preview;
    var lines = 1;
    for (var i = 0; i < text.length; i++) {
      if (text.codeUnitAt(i) == 0x0A) lines++;
    }
    return lines;
  }
}
