import 'dart:io';
import 'package:file_selector/file_selector.dart';
import 'package:path_provider/path_provider.dart';
import 'modules_repository.dart';
import '../models/module_models.dart';

class FileSelectorModulePackagePicker implements ModulePackagePicker {
  const FileSelectorModulePackagePicker();
  @override
  Future<SelectedModulePackage?> choose() async {
    final file = await openFile(
      acceptedTypeGroups: const [
        XTypeGroup(label: 'CopyPaste module', extensions: ['cpmodule']),
      ],
    );
    if (file == null) return null;
    final root = await getTemporaryDirectory();
    final directory = await root.createTemp('copypaste-module-');
    try {
      final path = '${directory.path}${Platform.pathSeparator}package.cpmodule';
      await file.saveTo(path);
      return SelectedModulePackage(
        path: path,
        dispose: () async {
          if (await directory.exists()) await directory.delete(recursive: true);
        },
      );
    } catch (_) {
      await directory.delete(recursive: true);
      rethrow;
    }
  }
}

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
    final directory = await root.createTemp('copypaste-input-');
    try {
      final path = '${directory.path}${Platform.pathSeparator}input.$extension';
      await selected.saveTo(path);
      return SelectedModuleInput(
        path: path,
        name: selected.name,
        dispose: () async {
          if (await directory.exists()) await directory.delete(recursive: true);
        },
      );
    } catch (_) {
      await directory.delete(recursive: true);
      rethrow;
    }
  }
}
