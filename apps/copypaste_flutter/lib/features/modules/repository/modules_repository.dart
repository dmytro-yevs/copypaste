import '../../../shared/forms/dynamic_form_draft.dart';
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

typedef ModuleInputPicker = DynamicFormInputPicker;

typedef SelectedModuleInput = SelectedDynamicFormInput;

class SelectedModulePackage {
  const SelectedModulePackage({required this.path, required this.dispose});
  final String path;
  final Future<void> Function() dispose;
}
