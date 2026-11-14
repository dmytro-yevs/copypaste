import 'dart:convert';

import '../../../generated/api.dart' as runtime;
import '../../../l10n/l10n.dart';
import '../models/module_models.dart';
import '../models/module_view.dart';
import 'modules_repository.dart';

/// Sends a `ModuleOperation` as its JSON object and returns the reply JSON.
typedef ModuleOperationSender =
    Future<String> Function(Map<String, Object?> operation);

/// The bridge adapter for the schema 6 operations (`SetEnabled` with
/// capabilities, `SetLocale`, `Prompts`, `AnswerPrompt`). TODO: call the
/// runtime bridge once it forwards these `ModuleOperation`s.
Future<String> sendModuleOperation(Map<String, Object?> operation) async =>
    throw ModulesException(AppStrings.current.modulesUnsupportedOperation);

class RuntimeModulesRepository implements ModulesRepository {
  RuntimeModulesRepository({ModuleOperationSender send = sendModuleOperation})
    : _send = send;

  final ModuleOperationSender _send;

  @override
  Future<List<InstalledModule>> list() => guardModuleRequest(() async {
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
          supportedPlatforms: List.unmodifiable(
            (module['supported_platforms'] as List<dynamic>? ?? const [])
                .cast<String>()
                .map(ModulePlatform.values.byName),
          ),
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
                _ => throw ModulesException(
                  AppStrings.current.modulesUnsupportedEvent,
                ),
              },
            ),
          ),
          capabilities: List.unmodifiable(
            (module['capabilities'] as List<dynamic>? ?? const []).map(
              (name) =>
                  ModuleCapability.fromWire(name as String) ??
                  (throw const FormatException()),
            ),
          ),
          commands: List.unmodifiable(
            (module['commands'] as List<dynamic>)
                .cast<Map<String, dynamic>>()
                .where((command) => command['hidden'] != true)
                .map(
                  (command) => ModuleCommand(
                    id: command['id'] as String,
                    title: command['title'] as String,
                    description: command['description'] as String,
                    arguments: _fields(command['arguments']),
                  ),
                ),
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
  Future<void> install(String packagePath) => guardModuleRequest(() async {
    await runtime.moduleInstall(packagePath: packagePath);
  });
  @override
  Future<void> setEnabled(
    String id,
    bool enabled, {
    List<ModuleCapability> capabilities = const [],
  }) => guardModuleRequest(
    () async => capabilities.isEmpty
        ? runtime.moduleSetEnabled(id: id, enabled: enabled)
        : await _send({
            'operation': 'set_enabled',
            'id': id,
            'enabled': enabled,
            'capabilities': [
              for (final capability in capabilities) capability.wireName,
            ],
          }),
  );

  @override
  Future<void> setLocale(String locale) => guardModuleRequest(
    () => _send({'operation': 'set_locale', 'locale': locale}),
  );
  @override
  Future<void> setPreferences(String id, Map<String, Object> values) =>
      guardModuleRequest(
        () => runtime.moduleSetPreferences(
          id: id,
          valuesJson: jsonEncode(values),
        ),
      );
  @override
  Future<void> remove(String id) =>
      guardModuleRequest(() => runtime.moduleRemove(id: id));
  @override
  Future<ModuleResult> invoke(
    String id,
    String command,
    Map<String, Object> arguments,
  ) => guardModuleRequest(() async {
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
      'view' => ModuleResult.view(
        _view(result['view'] as Map<String, dynamic>),
      ),
      'message' => ModuleResult(result['message'] as String),
      'data'
          when result['data'] is Map &&
              (result['data'] as Map)['message'] is String =>
        ModuleResult((result['data'] as Map)['message'] as String),
      _ => throw ModulesException(AppStrings.current.modulesUnsupportedResult),
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
      'device_set' => ModuleFieldKind.devices,
      'source_app_set' => ModuleFieldKind.applications,
      _ => throw ModulesException(AppStrings.current.modulesUnsupportedField),
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
      defaultValue: switch (kind) {
        ModuleFieldKind.file => '',
        ModuleFieldKind.devices ||
        ModuleFieldKind.applications => const <String>[],
        _ => field['default'] as Object,
      },
      acceptedExtensions: List.unmodifiable(
        (field['accepted_extensions'] as List<dynamic>? ?? const [])
            .cast<String>(),
      ),
      maxBytes: field['max_bytes'] as int? ?? 64 * 1024 * 1024,
      required: field['required'] as bool? ?? false,
    );
  }),
);

ModuleView _view(Map<String, dynamic> view) => ModuleView(
  sections: List.unmodifiable(
    (view['sections'] as List<dynamic>).cast<Map<String, dynamic>>().map(
      (section) => ModuleViewSection(
        title: section['title'] as String?,
        items: List.unmodifiable(
          (section['items'] as List<dynamic>).cast<Map<String, dynamic>>().map(
            (item) => switch (item['kind']) {
              'row' => ModuleViewRow(
                title: item['title'] as String,
                subtitle: item['subtitle'] as String?,
                actions: List.unmodifiable(
                  (item['actions'] as List<dynamic>? ?? const [])
                      .cast<Map<String, dynamic>>()
                      .map(
                        (action) => ModuleViewAction(
                          title: action['title'] as String,
                          command: action['command'] as String,
                          arguments: Map.unmodifiable(
                            (action['arguments'] as Map<String, dynamic>? ??
                                    const {})
                                .cast<String, Object>(),
                          ),
                          destructive: action['destructive'] as bool? ?? false,
                        ),
                      ),
                ),
              ),
              'block' => ModuleViewBlock(
                label: item['label'] as String?,
                text: item['text'] as String,
              ),
              _ => throw const FormatException(),
            },
          ),
        ),
      ),
    ),
  ),
);

/// Device names from history, whose ids device-set fields store.
class RuntimeModuleDeviceCatalog implements ModuleDeviceCatalog {
  const RuntimeModuleDeviceCatalog();

  @override
  Future<List<ModuleChoice>> devices() => guardModuleRequest(
    () async => [
      for (final device in (await runtime.historyFacets()).originDevices)
        ModuleChoice(id: device.id, title: device.label),
    ],
  );
}

Future<T> guardModuleRequest<T>(Future<T> Function() operation) async {
  try {
    return await operation();
  } on runtime.RuntimeError catch (error) {
    throw ModulesException(error.message);
  } on FormatException {
    throw ModulesException(AppStrings.current.modulesResponseInvalid);
  } on TypeError {
    throw ModulesException(AppStrings.current.modulesResponseInvalid);
  }
}
