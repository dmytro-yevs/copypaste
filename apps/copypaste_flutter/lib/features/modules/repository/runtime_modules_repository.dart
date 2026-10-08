import 'dart:convert';
import '../../../generated/api.dart' as runtime;
import '../models/module_models.dart';
import 'modules_repository.dart';

class RuntimeModulesRepository implements ModulesRepository {
  @override
  Future<List<InstalledModule>> list() => _guard(() async {
    final values = jsonDecode(await runtime.modulesList()) as List<dynamic>;
    return List.unmodifiable(
      values.map((value) {
        final module = value as Map<String, dynamic>;
        return InstalledModule(
          id: module['id'] as String,
          title: module['title'] as String,
          description: module['description'] as String,
          version: module['version'] as String,
          enabled: module['enabled'] as bool,
          sizeBytes: module['size_bytes'] as int,
          searchLanguageField: module['search_language_field'] as String?,
          searchModels: List.unmodifiable(
            (module['search_models'] as List<dynamic>? ?? const []).map((
              value,
            ) {
              final model = value as Map<String, dynamic>;
              return ModuleSearchModel(
                id: model['id'] as String,
                title: model['title'] as String,
                languages: List.unmodifiable(
                  (model['languages'] as List).cast<String>(),
                ),
                sizeBytes: model['size_bytes'] as int,
                available: model['available'] as bool,
              );
            }),
          ),
          error: module['error'] as String?,
          restartRequired: module['restart_required'] as bool? ?? false,
          events: List.unmodifiable(
            (module['events'] as List<dynamic>? ?? const []).map(
              (event) => switch (event) {
                'sms_received' => ModuleEventKind.smsReceived,
                _ => throw const ModulesException(
                  'The module uses an unsupported event.',
                ),
              },
            ),
          ),
          commands: List.unmodifiable(
            (module['commands'] as List<dynamic>).map((value) {
              final command = value as Map<String, dynamic>;
              return ModuleCommand(
                id: command['id'] as String,
                title: command['title'] as String,
                description: command['description'] as String,
                arguments: _fields(command['arguments']),
              );
            }),
          ),
          preferenceFields: _fields(module['preference_fields']),
          preferences: Map.unmodifiable(
            (module['preferences'] as Map<String, dynamic>)
                .cast<String, Object>(),
          ),
        );
      }),
    );
  });

  @override
  Future<void> install(String packagePath) => _guard(() async {
    await runtime.moduleInstall(packagePath: packagePath);
  });
  @override
  Future<void> setEnabled(String id, bool enabled) =>
      _guard(() => runtime.moduleSetEnabled(id: id, enabled: enabled));
  @override
  Future<void> setPreferences(String id, Map<String, Object> values) => _guard(
    () => runtime.moduleSetPreferences(id: id, valuesJson: jsonEncode(values)),
  );
  @override
  Future<void> remove(String id) => _guard(() => runtime.moduleRemove(id: id));
  @override
  Future<ModuleResult> invoke(
    String id,
    String command,
    Map<String, Object> arguments,
  ) => _guard(() async {
    final result =
        jsonDecode(
              await runtime.moduleInvoke(
                id: id,
                command: command,
                argumentsJson: jsonEncode(arguments),
              ),
            )
            as Map<String, dynamic>;
    return switch (result['kind']) {
      'text' => ModuleResult(result['text'] as String),
      'message' => ModuleResult(result['message'] as String),
      'data'
          when result['data'] is Map &&
              (result['data'] as Map)['message'] is String =>
        ModuleResult((result['data'] as Map)['message'] as String),
      _ => throw const ModulesException(
        'The module returned an unsupported result.',
      ),
    };
  });
}

List<ModuleField> _fields(dynamic value) => List.unmodifiable(
  (value as List<dynamic>).map((value) {
    final field = value as Map<String, dynamic>;
    final kind = switch (field['kind']) {
      'text' => ModuleFieldKind.text,
      'boolean' => ModuleFieldKind.boolean,
      'file' => ModuleFieldKind.file,
      'choices' => ModuleFieldKind.choices,
      _ => throw const ModulesException(
        'The module uses an unsupported field.',
      ),
    };
    return ModuleField(
      id: field['id'] as String,
      title: field['title'] as String,
      options: List.unmodifiable(
        (field['options'] as List<dynamic>? ?? const []).map((value) {
          final option = value as Map<String, dynamic>;
          return ModuleChoice(
            id: option['id'] as String,
            title: option['title'] as String,
          );
        }),
      ),
      kind: kind,
      secret: field['secret'] as bool? ?? false,
      defaultValue: kind == ModuleFieldKind.file
          ? ''
          : field['default'] as Object,
      acceptedExtensions: List.unmodifiable(
        (field['accepted_extensions'] as List<dynamic>? ?? const [])
            .cast<String>(),
      ),
      maxBytes: field['max_bytes'] as int? ?? 64 * 1024 * 1024,
      required: field['required'] as bool? ?? false,
    );
  }),
);

Future<T> _guard<T>(Future<T> Function() operation) async {
  try {
    return await operation();
  } on runtime.RuntimeError catch (error) {
    throw ModulesException(error.message);
  } on FormatException {
    throw const ModulesException('The module response is invalid.');
  } on TypeError {
    throw const ModulesException('The module response is invalid.');
  }
}
