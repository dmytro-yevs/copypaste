import 'dart:convert';

import '../../../generated/api.dart' as runtime;
import '../../../l10n/l10n.dart';
import '../../../shared/error_text.dart';
import '../models/module_models.dart';
import 'module_wire.dart';
import 'modules_repository.dart';

class RuntimeModulesRepository implements ModulesRepository {
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
                    arguments: ModuleWire.fields(command['arguments']),
                  ),
                ),
          ),
          surfaces: ModuleWire.surfaces(module['surfaces']),
          preferenceFields: ModuleWire.fields(module['preference_fields']),
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
    () => runtime.moduleSetEnabled(
      id: id,
      enabled: enabled,
      capabilities: [
        for (final capability in capabilities) capability.wireName,
      ],
    ),
  );

  @override
  Future<void> setLocale(String locale) =>
      guardModuleRequest(() => runtime.moduleSetLocale(locale: locale));
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
    return ModuleWire.output(result);
  });
}

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
    throw ModulesException(ErrorText.describe(error));
  } on FormatException {
    throw ModulesException(AppStrings.current.modulesResponseInvalid);
  } on TypeError {
    throw ModulesException(AppStrings.current.modulesResponseInvalid);
  }
}
