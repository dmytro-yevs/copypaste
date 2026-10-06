import 'package:copypaste_flutter/features/modules/models/module_models.dart';
import 'package:copypaste_flutter/features/modules/repository/modules_repository.dart';

const textArgument = ModuleField(
  id: 'text',
  title: 'Text',
  kind: ModuleFieldKind.text,
  defaultValue: '',
  required: true,
);
const uppercasePreference = ModuleField(
  id: 'uppercase',
  title: 'Uppercase',
  kind: ModuleFieldKind.boolean,
  defaultValue: false,
);
const testModule = InstalledModule(
  id: 'copypaste.text-tools',
  title: 'Text Tools',
  description: 'Transform text.',
  version: '1.0.0',
  enabled: true,
  sizeBytes: 10,
  commands: [
    ModuleCommand(
      id: 'transform',
      title: 'Transform text',
      description: 'Transform text.',
      arguments: [textArgument],
    ),
  ],
  preferenceFields: [uppercasePreference],
  preferences: {'uppercase': false},
);

class MemoryModulesRepository implements ModulesRepository {
  List<InstalledModule> modules = [];
  Object? failure;
  final calls = <String>[];
  Map<String, Object>? lastArguments;
  Map<String, Object>? lastPreferences;
  void _check() {
    if (failure case final error?) throw error;
  }

  @override
  Future<List<InstalledModule>> list() async => List.unmodifiable(modules);
  @override
  Future<void> install(String packagePath) async {
    calls.add('install');
    _check();
    modules = [testModule];
  }

  @override
  Future<void> setEnabled(String id, bool enabled) async {
    calls.add('enabled:$enabled');
    _check();
    final module = modules.single;
    modules = [
      InstalledModule(
        id: module.id,
        title: module.title,
        description: module.description,
        version: module.version,
        enabled: enabled,
        sizeBytes: module.sizeBytes,
        commands: module.commands,
        preferenceFields: module.preferenceFields,
        preferences: module.preferences,
      ),
    ];
  }

  @override
  Future<void> setPreferences(String id, Map<String, Object> values) async {
    calls.add('preferences');
    _check();
    lastPreferences = values;
  }

  @override
  Future<void> remove(String id) async {
    calls.add('remove');
    _check();
    modules = [];
  }

  @override
  Future<ModuleResult> invoke(
    String id,
    String command,
    Map<String, Object> arguments,
  ) async {
    calls.add('invoke');
    _check();
    lastArguments = arguments;
    return ModuleResult((arguments['text'] as String).toUpperCase());
  }
}

class MemoryModulePicker implements ModulePackagePicker {
  bool cancel = false;
  int disposedPackages = 0;
  @override
  Future<SelectedModulePackage?> choose() async => cancel
      ? null
      : SelectedModulePackage(
          path: '/private/test.cpmodule',
          dispose: () async {
            disposedPackages++;
          },
        );
}
