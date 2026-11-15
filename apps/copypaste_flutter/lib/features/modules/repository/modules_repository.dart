import '../../../shared/forms/dynamic_form_draft.dart';
import '../models/module_models.dart';
import '../models/module_output.dart';
import '../models/module_prompt.dart';

abstract interface class ModulesRepository {
  Future<List<InstalledModule>> list();
  Future<void> install(String packagePath);

  /// Enabling echoes every declared capability the user confirmed.
  Future<void> setEnabled(
    String id,
    bool enabled, {
    List<ModuleCapability> capabilities = const [],
  });

  /// Synced fields are routed to synced storage by the host.
  Future<void> setPreferences(String id, Map<String, Object> values);
  Future<void> remove(String id);
  Future<ModuleResult> invoke(
    String id,
    String command,
    Map<String, Object> arguments,
  );

  /// The app language tag, such as `uk` or `pt-BR`, for module strings.
  Future<void> setLocale(String locale);
}

/// Questions modules are waiting for the user to answer.
abstract interface class ModulePromptsRepository {
  /// Fires when the pending set may have changed, including on reconnect.
  Stream<void> changes();
  Future<List<ModulePrompt>> pending();

  /// A null [choice] dismisses the prompt.
  Future<void> answer(String promptId, String? choice);
}

/// A system notification for a prompt that arrives while the app is hidden;
/// activating it reports the prompt id.
abstract interface class ModulePromptNotifier {
  Stream<String> get opened;
  Future<void> show(ModulePrompt prompt);
  Future<void> dismiss();
}

/// Devices a device-set field can name, by the ids history uses.
abstract interface class ModuleDeviceCatalog {
  Future<List<ModuleChoice>> devices();
}

typedef ModuleInputPicker = DynamicFormInputPicker;

typedef SelectedModuleInput = SelectedDynamicFormInput;

class SelectedModulePackage {
  const SelectedModulePackage({required this.path, required this.dispose});
  final String path;
  final Future<void> Function() dispose;
}

/// Lets the user choose a module package file to install.
abstract interface class ModulePackagePicker {
  /// A private copy of the chosen package, or null when cancelled.
  Future<SelectedModulePackage?> choosePackage();
}

/// Saves a file a module wrote where the user chooses, then deletes the
/// module's copy.
abstract interface class ModuleFileSaver {
  /// False when the user cancelled; the module's copy is deleted either way.
  Future<bool> save(String moduleId, ModuleSavedFile file);
}
