import '../../../platform/files/history_file_picker.dart';
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
  if (Platform.isAndroid) {
    TemporaryDirectoryLease? lease;
    try {
      final copy = await SystemHistoryFilePicker.copyChosenAndroidDocument(
        maxBytes: maxBytes,
        destination: (name) async {
          final extension = _acceptedExtension(name, extensions, label);
          lease = await TemporaryDirectoryLease.create(
            await getTemporaryDirectory(),
            prefix,
          );
          return '${lease!.directory.path}/input.$extension';
        },
      );
      if (copy == null) return null;
      return (path: copy.path, name: copy.name, dispose: lease!.dispose);
    } on PlatformFileException catch (error) {
      await lease?.dispose();
      if (!error.tooLarge) rethrow;
      throw ModulesException(AppStrings.current.modulesFileNotAccepted(label));
    } catch (_) {
      await lease?.dispose();
      rethrow;
    }
  }
  final selected = await openFile(
    acceptedTypeGroups: [XTypeGroup(label: label, extensions: extensions)],
  );
  if (selected == null) return null;
  final extension = _acceptedExtension(selected.name, extensions, label);
  if (await selected.length() > maxBytes) {
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

String _acceptedExtension(String name, List<String> extensions, String label) {
  final extension = name.split('.').last.toLowerCase();
  if (!extensions.contains(extension)) {
    throw ModulesException(AppStrings.current.modulesFileNotAccepted(label));
  }
  return extension;
}
