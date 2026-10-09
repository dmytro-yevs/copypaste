enum ModulesLoadState { loading, ready, error }

enum ModuleFieldKind { text, boolean, file, choices }

enum ModuleEventKind { smsReceived }

enum ModulePlatform {
  macos('macOS'),
  windows('Windows'),
  android('Android');

  const ModulePlatform(this.label);
  final String label;
}

String formatModulePlatforms(List<ModulePlatform> platforms) =>
    platforms.isEmpty
    ? 'Platforms: Unknown'
    : 'Platforms: ${ModulePlatform.values.where(platforms.contains).map((platform) => platform.label).join(' · ')}';

class ModuleField {
  const ModuleField({
    required this.id,
    required this.title,
    required this.kind,
    required this.defaultValue,
    this.required = false,
    this.secret = false,
    this.options = const [],
    this.acceptedExtensions = const [],
    this.maxBytes = 64 * 1024 * 1024,
  });
  final String id;
  final String title;
  final ModuleFieldKind kind;
  final Object defaultValue;
  final bool required;
  final bool secret;
  final List<ModuleChoice> options;
  final List<String> acceptedExtensions;
  final int maxBytes;
}

class ModuleChoice {
  const ModuleChoice({required this.id, required this.title});
  final String id;
  final String title;
}

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
}

class ModuleResult {
  const ModuleResult(this.text);
  final String text;
}

class ModulesException implements Exception {
  const ModulesException(this.message);
  final String message;
  @override
  String toString() => message;
}
