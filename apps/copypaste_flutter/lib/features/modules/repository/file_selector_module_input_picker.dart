import '../../../platform/files/temporary_artifacts.dart';
import 'dart:io';
import 'package:file_selector/file_selector.dart';
import 'package:path_provider/path_provider.dart';
import 'modules_repository.dart';
import '../models/module_models.dart';

class FileSelectorModuleInputPicker implements ModuleInputPicker {
  const FileSelectorModuleInputPicker();
  @override
  Future<SelectedModuleInput?> chooseInput(ModuleField field) async {
    final selected = await openFile(
      acceptedTypeGroups: [
        XTypeGroup(label: field.title, extensions: field.acceptedExtensions),
      ],
    );
    if (selected == null) return null;
    final extension = selected.name.split('.').last.toLowerCase();
    if (!field.acceptedExtensions.contains(extension) ||
        await selected.length() > field.maxBytes) {
      throw ModulesException('${field.title} is not an accepted file.');
    }
    final root = await getTemporaryDirectory();
    final lease = await TemporaryDirectoryLease.create(
      root,
      'copypaste-input-',
    );
    final directory = lease.directory;
    try {
      final path = '${directory.path}${Platform.pathSeparator}input.$extension';
      await selected.saveTo(path);
      return SelectedModuleInput(
        path: path,
        name: selected.name,
        dispose: lease.dispose,
      );
    } catch (_) {
      await lease.dispose();
      rethrow;
    }
  }
}
