import 'dart:io';

import 'package:file_selector/file_selector.dart';

import '../../../platform/files/platform_file_gateway.dart';
import '../models/history_models.dart';
import 'history_file_downloader.dart';

class FileSelectorHistoryFileDownloader implements HistoryFileDownloader {
  const FileSelectorHistoryFileDownloader({
    this.fileGateway = const PlatformFileGateway(),
  });

  final PlatformFileGateway fileGateway;

  @override
  Future<String?> chooseDestination(HistoryFileDetails file) async {
    final suggestedName = file.name?.trim().isNotEmpty == true
        ? file.name!.trim()
        : 'copypaste-file';
    if (Platform.isAndroid) {
      return fileGateway.androidStagingPath(suggestedName);
    }
    return (await getSaveLocation(suggestedName: suggestedName))?.path;
  }

  @override
  Future<void> presentSavedFile(String path, HistoryFileDetails file) =>
      fileGateway.presentCreatedFile(
        path,
        mimeType: file.mimeType ?? 'application/octet-stream',
      );
}
