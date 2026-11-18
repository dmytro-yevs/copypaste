import 'dart:io';

import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:file_selector/file_selector.dart';
import 'package:flutter/services.dart';
import 'package:mime/mime.dart';

import '../../features/history/repository/history_file_importer.dart';
import '../../shared/error_text.dart';

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
    try {
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
              sizeBytes: (file['sizeBytes'] as num?)?.toInt(),
              prepare: (maxBytes) async =>
                  (await androidChannel.invokeMethod<String>('materialize', {
                    'token': token,
                    'maxBytes': maxBytes,
                  }))!,
              dispose: () => androidChannel.invokeMethod<void>('release', {
                'token': token,
              }),
            );
          })
          .toList(growable: false);
    } on PlatformException catch (error) {
      throw HistoryFileOperationException(
        error.message ?? AppStrings.current.platformFilesSelectionFailed,
      );
    }
  }

  /// Lets the user choose one Android document and streams it to the cache
  /// path [destination] returns for its name. A document whose reported size
  /// exceeds [maxBytes] is rejected before any byte is read; without
  /// [maxBytes] the copy is bounded by the free space at the destination.
  /// Returns null when the picker is cancelled.
  static Future<({String name, String path})?> copyChosenAndroidDocument({
    required Future<String> Function(String name) destination,
    int? maxBytes,
  }) async {
    final selected = await _platformCall(
      () => androidChannel.invokeListMethod<Object?>('chooseFiles', {
        'oneShot': true,
      }),
    );
    if (selected == null || selected.isEmpty) return null;
    final reply = selected.first;
    final token = reply is Map ? reply['token'] : null;
    try {
      final file = Map<String, Object?>.from(reply! as Map);
      final name = file['name']! as String;
      final size = (file['sizeBytes'] as num?)?.toInt();
      if (maxBytes != null && size != null && size > maxBytes) {
        throw PlatformFileException(
          AppStrings.current.platformFilesSelectionFailed,
          tooLarge: true,
        );
      }
      final path = await destination(name);
      await _platformCall(
        () => androidChannel.invokeMethod<String>('materialize', {
          'token': token! as String,
          'maxBytes': ?maxBytes,
          'destination': path,
        }),
      );
      return (name: name, path: path);
    } finally {
      if (token is String) {
        try {
          await androidChannel.invokeMethod<void>('release', {'token': token});
        } catch (_) {
          // A lingering selection holds only a URI; the copy's outcome stands.
        }
      }
    }
  }

  static Future<T> _platformCall<T>(Future<T> Function() call) async {
    try {
      return await call();
    } on PlatformException catch (error) {
      throw PlatformFileException(
        error.message ?? AppStrings.current.platformFilesSelectionFailed,
        tooLarge: error.code == 'file_too_large',
      );
    }
  }

  static HistoryImportFile fromDesktopFile(
    XFile file, {
    bool temporary = false,
  }) {
    // Windows accepts both separators, while cross_file splits on '\\' only.
    final name = Platform.isWindows
        ? file.name.replaceAll('\\', '/').split('/').last
        : file.name;
    return HistoryImportFile(
      name: name,
      mimeType:
          file.mimeType ?? lookupMimeType(name) ?? 'application/octet-stream',
      sourceReference: temporary ? null : file.path,
      prepare: (_) async {
        if (await FileSystemEntity.type(file.path) !=
            FileSystemEntityType.file) {
          throw HistoryFileOperationException(
            AppStrings.current.platformFilesOnlyRegular,
          );
        }
        return file.path;
      },
      dispose: () async {},
    );
  }
}

/// A path-free failure of the native document picker or its copy.
class PlatformFileException implements UserFacingException {
  const PlatformFileException(this.message, {this.tooLarge = false});
  @override
  final String message;

  /// The document exceeds the size limit the caller passed.
  final bool tooLarge;
}
