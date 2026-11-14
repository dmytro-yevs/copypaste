import '../../../l10n/l10n.dart';
import '../../../shared/forms/dynamic_form_draft.dart';
import 'module_view.dart';

enum ModulesLoadState { loading, ready, error }

typedef ModuleFieldKind = DynamicFormFieldKind;

enum ModuleEventKind { smsReceived }

/// A host service a module declares; the user confirms them on enabling.
enum ModuleCapability {
  historyRead('history_read'),
  clipboardWrite('clipboard_write'),
  libraryWrite('library_write'),
  snippets('snippets'),
  syncedRecords('synced_records'),
  localState('local_state'),
  prompts('prompts'),
  endpoints('endpoints');

  const ModuleCapability(this.wireName);
  final String wireName;

  static ModuleCapability? fromWire(String name) =>
      values.where((value) => value.wireName == name).firstOrNull;
}

enum ModulePlatform {
  macos('macOS'),
  windows('Windows'),
  android('Android'),
  linux('Linux');

  const ModulePlatform(this.label);
  final String label;
}

String formatModulePlatforms(List<ModulePlatform> platforms) =>
    platforms.isEmpty
    ? AppStrings.current.modulesPlatformsUnknown
    : AppStrings.current.modulesPlatforms(
        ModulePlatform.values
            .where(platforms.contains)
            .map((platform) => platform.label)
            .join(' · '),
      );

typedef ModuleField = DynamicFormField;

typedef ModuleChoice = DynamicFormChoice;

class ModuleSearchModel {
  const ModuleSearchModel({
    required this.id,
    required this.title,
    required this.languages,
    required this.sizeBytes,
    required this.available,
  });
  final String id;
  final String title;
  final List<String> languages;
  final int sizeBytes;
  final bool available;

  static ModuleSearchModel? forLanguages(
    List<ModuleSearchModel> models,
    Iterable<String> languages,
  ) {
    if (languages.isEmpty) return null;
    ModuleSearchModel? selected;
    for (final model in models) {
      if (languages.every(model.languages.contains) &&
          (selected == null || model.sizeBytes < selected.sizeBytes)) {
        selected = model;
      }
    }
    return selected;
  }
}

class ModuleCommand {
  const ModuleCommand({
    required this.id,
    required this.title,
    required this.description,
    required this.arguments,
  });
  final String id;
  final String title;
  final String description;
  final List<ModuleField> arguments;
}

class InstalledModule {
  const InstalledModule({
    required this.id,
    required this.title,
    required this.description,
    required this.version,
    required this.enabled,
    required this.sizeBytes,
    required this.commands,
    required this.preferenceFields,
    required this.preferences,
    this.supportedPlatforms = const [],
    this.searchModels = const [],
    this.searchLanguageField,
    this.error,
    this.restartRequired = false,
    this.events = const [],
    this.capabilities = const [],
  });
  final String id;
  final String title;
  final String description;
  final String version;
  final bool enabled;
  final int sizeBytes;
  final List<ModuleCommand> commands;
  final List<ModuleField> preferenceFields;
  final Map<String, Object> preferences;
  final List<ModulePlatform> supportedPlatforms;
  final List<ModuleSearchModel> searchModels;
  final String? searchLanguageField;
  final String? error;
  final bool restartRequired;
  final List<ModuleEventKind> events;
  final List<ModuleCapability> capabilities;

  /// The search model that serves the languages chosen in [values].
  ModuleSearchModel? searchModelFor(Map<String, Object> values) {
    final languages = values[searchLanguageField];
    return languages is List
        ? ModuleSearchModel.forLanguages(searchModels, languages.cast<String>())
        : null;
  }

  /// Preferences are complete only when a search model serves them.
  bool acceptsPreferences(Map<String, Object> values) =>
      searchModels.isEmpty || searchModelFor(values) != null;
}

/// A command result: text to show, or a [view] the host renders in place.
class ModuleResult {
  const ModuleResult(this.text) : view = null;
  const ModuleResult.view(ModuleView this.view) : text = '';
  final String text;
  final ModuleView? view;
}

class ModulesException implements UserFacingException {
  const ModulesException(this.message);
  @override
  final String message;
  @override
  String toString() => message;
}
