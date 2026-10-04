import 'dart:io';

import 'package:flutter/services.dart';
import 'package:path_provider/path_provider.dart';

/// Shared native presentation for files created by CopyPaste.
class PlatformFileGateway {
  const PlatformFileGateway();

  static const _androidChannel = MethodChannel(
    'com.copypaste.app/settings_files',
  );

  Future<String> androidStagingPath(String suggestedName) async {
    final root = await getTemporaryDirectory();
    final directory = Directory('${root.path}/clipboard');
    await directory.create(recursive: true);
    await _removeExpiredStagingFiles(directory);
    final safeName = suggestedName
        .split(RegExp(r'[/\\]'))
        .last
        .replaceAll(RegExp(r'[^A-Za-z0-9._ -]'), '_')
        .trim();
    final filename = safeName.isEmpty ? 'copypaste-file' : safeName;
    return '${directory.path}/copypaste-${DateTime.now().microsecondsSinceEpoch}-$filename';
  }

  Future<void> presentCreatedFile(
    String path, {
    required String mimeType,
  }) async {
    if (!Platform.isAndroid) return;
    await _androidChannel.invokeMethod<void>('shareFile', {
      'path': path,
      'mimeType': mimeType,
    });
  }

  Future<void> _removeExpiredStagingFiles(Directory directory) async {
    final cutoff = DateTime.now().subtract(const Duration(days: 1));
    await for (final entry in directory.list()) {
      if (entry is! File ||
          !entry.path.split('/').last.startsWith('copypaste-')) {
        continue;
      }
      try {
        if ((await entry.stat()).modified.isBefore(cutoff)) {
          await entry.delete();
        }
      } on FileSystemException {
        // Cache cleanup is best-effort and must not block a new file.
      }
    }
  }
}
