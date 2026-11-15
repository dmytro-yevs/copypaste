import '../../../platform/files/temporary_artifacts.dart';
import '../../../l10n/l10n.dart';

import 'dart:io';

import 'package:file_selector/file_selector.dart';
import 'package:path_provider/path_provider.dart';

import 'github_module_marketplace_repository.dart';
import 'modules_repository.dart';
import '../models/module_models.dart';

class FileSelectorModuleInputPicker implements ModuleInputPicker {
  const FileSelectorModuleInputPicker();
  @override
  Future<SelectedModuleInput?> chooseInput(ModuleField field) async {
    final copy = await _choosePrivateCopy(
      label: field.title,
      extensions: field.acceptedExtensions,
      maxBytes: field.maxBytes,
      prefix: 'copypaste-input-',
    );
    if (copy == null) return null;
    return SelectedModuleInput(
      path: copy.path,
      name: copy.name,
      dispose: copy.dispose,
    );
  }
}

class FileSelectorModulePackagePicker implements ModulePackagePicker {
  const FileSelectorModulePackagePicker();
  @override
  Future<SelectedModulePackage?> choosePackage() async {
    final copy = await _choosePrivateCopy(
      label: AppStrings.current.modulesPackageFileType,
      extensions: const ['cpmodule'],
      maxBytes: maximumModulePackageBytes,
      prefix: 'copypaste-module-',
    );
    if (copy == null) return null;
    return SelectedModulePackage(path: copy.path, dispose: copy.dispose);
  }
}

/// Copies the chosen file into a private temporary directory, so the
/// daemon reads it without access to the folder the user picked from.
Future<({String path, String name, Future<void> Function() dispose})?>
_choosePrivateCopy({
  required String label,
  required List<String> extensions,
  required int maxBytes,
  required String prefix,
}) async {
  final selected = await openFile(
    acceptedTypeGroups: [XTypeGroup(label: label, extensions: extensions)],
  );
  if (selected == null) return null;
  final extension = selected.name.split('.').last.toLowerCase();
  if (!extensions.contains(extension) || await selected.length() > maxBytes) {
    throw ModulesException(AppStrings.current.modulesFileNotAccepted(label));
  }
  final root = await getTemporaryDirectory();
  final lease = await TemporaryDirectoryLease.create(root, prefix);
  try {
    final path =
        '${lease.directory.path}${Platform.pathSeparator}input.$extension';
    await selected.saveTo(path);
    return (path: path, name: selected.name, dispose: lease.dispose);
  } catch (_) {
    await lease.dispose();
    rethrow;
  }
}
