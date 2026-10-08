import 'dart:io';
import 'package:file_selector/file_selector.dart';
import 'package:flutter/services.dart';
import 'package:mime/mime.dart';
import '../../features/history/repository/history_file_importer.dart';

/// Uses native document descriptors on Android to keep the provider's name and
/// URI and materialize only the file currently being imported.
class SystemHistoryFilePicker implements HistoryFilePicker {
  const SystemHistoryFilePicker();
  static const androidChannel = MethodChannel(
    'com.copypaste.app/history_files',
  );

  @override
  Future<List<HistoryImportFile>> chooseFiles() async {
    if (!Platform.isAndroid) {
      return (await openFiles()).map(fromDesktopFile).toList(growable: false);
    }
    final selected =
        await androidChannel.invokeListMethod<Object?>('chooseFiles') ??
        const [];
    return selected
        .map((value) {
          final file = Map<String, Object?>.from(value! as Map);
          final token = file['token']! as String;
          return HistoryImportFile(
            name: file['name']! as String,
            mimeType: file['mimeType']! as String,
            sourceReference: file['uri']! as String,
            prepare: (maxBytes) async =>
                (await androidChannel.invokeMethod<String>('materialize', {
                  'token': token,
                  'maxBytes': maxBytes,
                }))!,
            dispose: () =>
                androidChannel.invokeMethod<void>('release', {'token': token}),
          );
        })
        .toList(growable: false);
  }

  static HistoryImportFile fromDesktopFile(
    XFile file, {
    bool temporary = false,
  }) => HistoryImportFile(
    name: file.name,
    mimeType:
        file.mimeType ??
        lookupMimeType(file.name) ??
        'application/octet-stream',
    sourceReference: temporary ? null : file.path,
    prepare: (_) async {
      if (await FileSystemEntity.type(file.path) != FileSystemEntityType.file) {
        throw const FileSystemException('Only regular files can be imported.');
      }
      return file.path;
    },
    dispose: () async {},
  );
}
