enum ModulesLoadState { loading, ready, error }

enum ModuleFieldKind { text, boolean, file }

class ModuleField {
  const ModuleField({
    required this.id,
    required this.title,
    required this.kind,
    required this.defaultValue,
    this.required = false,
    this.acceptedExtensions = const [],
    this.maxBytes = 64 * 1024 * 1024,
  });
  final String id;
  final String title;
  final ModuleFieldKind kind;
  final Object defaultValue;
  final bool required;
  final List<String> acceptedExtensions;
  final int maxBytes;
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
    this.error,
    this.restartRequired = false,
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
  final String? error;
  final bool restartRequired;
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
