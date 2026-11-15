import 'dart:io';

import 'package:file_selector/file_selector.dart';

import '../../../generated/api.dart' as runtime;
import '../../../platform/files/platform_file_gateway.dart';
import '../models/history_models.dart';
import 'history_file_downloader.dart';

/// The save folder from Settings; empty when every download asks.
Future<String> runtimeSaveFolder() async {
  try {
    return (await runtime.getRuntimeSettings()).fileSaveFolder;
  } on Object {
    return '';
  }
}

class FileSelectorHistoryFileDownloader implements HistoryFileDownloader {
  const FileSelectorHistoryFileDownloader({
    this.fileGateway = const PlatformFileGateway(),
    this.saveFolder = runtimeSaveFolder,
  });

  final PlatformFileGateway fileGateway;
  final Future<String> Function() saveFolder;

  /// A chosen save folder is returned as is; the runtime names the files
  /// inside it. Otherwise a multi-file clip is saved into an existing folder.
  @override
  Future<String?> chooseDestination(HistoryFileDetails file) async {
    final folder = await saveFolder();
    if (folder.isNotEmpty) return folder;
    if (file.isSet) {
      if (Platform.isAndroid) return fileGateway.androidStagingFolder();
      return getDirectoryPath();
    }
    final suggestedName = file.name?.trim().isNotEmpty == true
        ? file.name!.trim()
        : 'copypaste-file';
    if (Platform.isAndroid) {
      return fileGateway.androidStagingPath(suggestedName, persistent: true);
    }
    return (await getSaveLocation(suggestedName: suggestedName))?.path;
  }

  @override
  Future<void> presentSavedFile(String path, HistoryFileDetails file) async {
    // A file saved to the chosen folder is already where the user wants it.
    if (path == await saveFolder()) return;
    if (!file.isSet) {
      return fileGateway.presentCreatedFile(
        path,
        mimeType: file.mimeType ?? 'application/octet-stream',
      );
    }
    if (!Platform.isAndroid) return fileGateway.openFile(path);
    await fileGateway.presentCreatedFiles([
      await for (final entry in Directory(path).list(followLinks: false))
        if (entry is File) entry.path,
    ]);
  }
}
