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
    HistoryClipKind.image => LucideIcons.image,
    HistoryClipKind.file => LucideIcons.file,
    HistoryClipKind.other => LucideIcons.box,
  };
}
