import 'dart:io';

import 'package:dbus/dbus.dart';
import 'package:flutter/services.dart';
import 'package:path_provider/path_provider.dart';
import 'package:mime/mime.dart';

import 'file_names.dart';

/// Shared native presentation for files created by CopyPaste.
class PlatformFileGateway {
  const PlatformFileGateway();

  static const _androidChannel = MethodChannel(
    'com.copypaste.app/settings_files',
  );

  Future<String> androidStagingPath(
    String suggestedName, {
    bool persistent = false,
  }) async {
    final root = persistent
        ? await getApplicationSupportDirectory()
        : await getTemporaryDirectory();
    final directory = Directory(
      '${root.path}/${persistent ? "copypaste-downloads" : "clipboard"}',
    );
    // AndroidClipboardCache owns expiry of the flat cache/clipboard files.
    await directory.create(recursive: true);
    // The timestamp prefix stays below 40 bytes of the 255-byte name limit.
    final filename = safeFileName(
      suggestedName,
      maxBytes: 215,
      fallback: 'copypaste-file',
    );
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

  Future<void> openFile(String path) async {
    if (Platform.isAndroid) {
      await _androidChannel.invokeMethod<void>('openFile', {
        'path': path,
        'mimeType': lookupMimeType(path) ?? 'application/octet-stream',
      });
      return;
    }
    if (Platform.isLinux && await _showItemOnLinux(path)) return;
    final result = Platform.isMacOS
        ? await Process.run('open', ['-R', path])
        : Platform.isWindows
        ? await Process.run('explorer.exe', ['/select,', path])
        : await Process.run('xdg-open', [File(path).parent.path]);
    if (result.exitCode != 0) {
      throw const FileSystemException('File could not be opened');
    }
  }

  /// Selects the file through the freedesktop file-manager interface. Returns
  /// false when no file manager implements it, so the folder opens instead.
  Future<bool> _showItemOnLinux(String path) async {
    final client = DBusClient.session();
    try {
      await client.callMethod(
        destination: 'org.freedesktop.FileManager1',
        path: DBusObjectPath('/org/freedesktop/FileManager1'),
        interface: 'org.freedesktop.FileManager1',
        name: 'ShowItems',
        values: [
          DBusArray.string([Uri.file(path).toString()]),
          const DBusString(''),
        ],
        replySignature: DBusSignature.empty,
      );
      return true;
    } on Object {
      return false;
    } finally {
      await client.close();
    }
  }
}
