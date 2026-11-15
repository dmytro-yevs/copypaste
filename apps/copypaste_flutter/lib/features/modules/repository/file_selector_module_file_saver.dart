import 'dart:io';

import 'package:file_selector/file_selector.dart';
import 'package:flutter/foundation.dart';
import 'package:mime/mime.dart';
import 'package:path_provider/path_provider.dart';

import '../../../l10n/l10n.dart';
import '../../../platform/files/platform_file_gateway.dart';
import '../../../platform/files/runtime_data_directory.dart';
import '../models/module_models.dart';
import '../models/module_output.dart';
import 'modules_repository.dart';

/// Asks for a destination with the system save dialog, or shares the file on
/// Android, where the runtime keeps module data in the app's files.
class FileSelectorModuleFileSaver implements ModuleFileSaver {
  const FileSelectorModuleFileSaver({
    this.fileGateway = const PlatformFileGateway(),
    this.dataDirectory = moduleDataDirectory,
  });

  final PlatformFileGateway fileGateway;
  final Future<Directory> Function(String moduleId) dataDirectory;

  @override
  Future<bool> save(String moduleId, ModuleSavedFile file) async {
    final source = File(
      moduleFilePath((await dataDirectory(moduleId)).path, file.path),
    );
    try {
      if (!await source.exists()) {
        throw ModulesException(AppStrings.current.modulesResponseInvalid);
      }
      final destination = Platform.isAndroid
          ? await fileGateway.androidStagingPath(file.name, persistent: true)
          : (await getSaveLocation(suggestedName: file.name))?.path;
      if (destination == null) return false;
      await source.copy(destination);
      await fileGateway.presentCreatedFile(
        destination,
        mimeType: lookupMimeType(file.name) ?? 'application/octet-stream',
      );
      return true;
    } finally {
      if (await source.exists()) await source.delete();
    }
  }
}

/// Where the runtime keeps the data of module [moduleId].
Future<Directory> moduleDataDirectory(String moduleId) async {
  final runtime = Platform.isAndroid
      ? Directory('${(await getApplicationSupportDirectory()).path}/runtime')
      : await runtimeDataDirectory();
  return Directory('${runtime.path}/modules/data/$moduleId');
}

/// [relative] inside [root]; refuses a path that leaves it.
@visibleForTesting
String moduleFilePath(String root, String relative) {
  final parts = relative.split(RegExp(r'[\\/]'));
  if (relative.isEmpty ||
      relative.startsWith('/') ||
      parts.any((part) => part.isEmpty || part == '.' || part == '..')) {
    throw ModulesException(AppStrings.current.modulesResponseInvalid);
  }
  return [root, ...parts].join(Platform.pathSeparator);
}
