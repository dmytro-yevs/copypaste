import 'dart:io';
import 'package:desktop_drop/desktop_drop.dart' as native;
import 'package:flutter/widgets.dart';
import '../../features/history/repository/history_file_importer.dart';
import 'history_file_picker.dart';

/// Typed boundary for external OS file drops, independent of Flutter's internal
/// pinned-row DragTarget. Text/URL payloads and directories are not imported.
class HistoryFileDropTarget extends StatelessWidget {
  const HistoryFileDropTarget({
    super.key,
    required this.enabled,
    required this.onFiles,
    required this.onHover,
    required this.child,
  });
  final bool enabled;
  final ValueChanged<List<HistoryImportFile>> onFiles;
  final ValueChanged<bool> onHover;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    if (!Platform.isMacOS && !Platform.isWindows && !Platform.isLinux) {
      return child;
    }
    return native.DropTarget(
      enable: enabled,
      onDragEntered: (_) => onHover(true),
      onDragExited: (_) => onHover(false),
      onDragDone: (details) {
        onHover(false);
        final files = details.files
            .whereType<native.DropItemFile>()
            .map((file) => _fromDrop(file))
            .toList(growable: false);
        if (files.isNotEmpty) onFiles(files);
      },
      child: child,
    );
  }

  static HistoryImportFile _fromDrop(native.DropItemFile file) {
    final input = SystemHistoryFilePicker.fromDesktopFile(
      file,
      temporary: file.fromPromise,
    );
    var accessStarted = false;
    final bookmark = file.extraAppleBookmark;
    return HistoryImportFile(
      name: input.name,
      mimeType: input.mimeType,
      sourceReference: input.sourceReference,
      prepare: (maxBytes) async {
        if (Platform.isMacOS && bookmark != null && bookmark.isNotEmpty) {
          accessStarted = await native.DesktopDrop.instance
              .startAccessingSecurityScopedResource(bookmark: bookmark);
        }
        return input.prepare(maxBytes);
      },
      dispose: () async {
        try {
          if (accessStarted) {
            await native.DesktopDrop.instance
                .stopAccessingSecurityScopedResource(bookmark: bookmark!);
          }
        } finally {
          if (file.fromPromise && await File(file.path).exists()) {
            await File(file.path).delete();
          }
        }
      },
    );
  }
}
