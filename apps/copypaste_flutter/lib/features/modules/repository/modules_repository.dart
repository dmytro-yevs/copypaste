import '../models/module_models.dart';

abstract interface class ModulesRepository {
  Future<List<InstalledModule>> list();
  Future<void> install(String packagePath);
  Future<void> setEnabled(String id, bool enabled);
  Future<void> setPreferences(String id, Map<String, Object> values);
  Future<void> remove(String id);
  Future<ModuleResult> invoke(
    String id,
    String command,
    Map<String, Object> arguments,
  );
}

abstract interface class ModulePackagePicker {
  Future<SelectedModulePackage?> choose();
}

abstract interface class ModuleInputPicker {
  Future<SelectedModuleInput?> chooseInput(ModuleField field);
}

class SelectedModuleInput {
  const SelectedModuleInput({
    required this.path,
    required this.name,
    required this.dispose,
  });
  final String path;
  final String name;
  final Future<void> Function() dispose;
}

class SelectedModulePackage {
  const SelectedModulePackage({required this.path, required this.dispose});
  final String path;
  final Future<void> Function() dispose;
}
