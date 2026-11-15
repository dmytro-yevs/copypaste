import 'dart:io';

/// Opens a folder in the platform file manager.
abstract interface class FolderReveal {
  /// Finder and File Explorer can show a folder; Android app storage is
  /// private to the app, so there is nothing to show it in.
  bool get supported;

  Future<void> reveal(String path);
}

class SystemFolderReveal implements FolderReveal {
  const SystemFolderReveal();

  @override
  bool get supported => Platform.isMacOS || Platform.isWindows;

  @override
  Future<void> reveal(String path) async {
    final directory = Directory(path);
    await directory.create(recursive: true);
    if (Platform.isMacOS) {
      final result = await Process.run('open', [directory.path]);
      if (result.exitCode != 0) throw FileSystemException('open', path);
    } else if (Platform.isWindows) {
      // Explorer reports exit code 1 even when it opened the folder.
      await Process.run('explorer.exe', [directory.path]);
    } else {
      throw UnsupportedError('No file manager is available.');
    }
  }
}
