import 'dart:async';

import 'package:pub_semver/pub_semver.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:copypaste_flutter/features/modules/controller/modules_controller.dart';
import 'package:copypaste_flutter/features/modules/models/module_models.dart';
import 'package:copypaste_flutter/features/modules/models/module_marketplace_models.dart';
import 'package:copypaste_flutter/features/modules/models/module_operation.dart';
import 'package:copypaste_flutter/features/modules/repository/modules_repository.dart';

import 'modules_test_support.dart';

void main() {
  test('installs a chosen package file and shows installed modules', () async {
    final repository = MemoryModulesRepository();
    final picker = MemoryModulePackagePicker()..path = null;
    final controller = ModulesController(
      repository: repository,
      marketplace: MemoryModuleMarketplace(),
      packagePicker: picker,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await controller.installFromFile();
    expect(repository.calls, isEmpty);
    expect(controller.section, ModulesSection.marketplace);

    picker.path = '/private/chosen.cpmodule';
    await controller.installFromFile();
    expect(repository.lastPackagePath, '/private/chosen.cpmodule');
    expect(picker.disposed, 1);
    expect(controller.modules, [testModule]);
    expect(controller.section, ModulesSection.installed);
  });

  test('module removal reports failure before a successful retry', () async {
    final repository = MemoryModulesRepository()..modules = [testModule];
    final controller = ModulesController(
      repository: repository,
      marketplace: MemoryModuleMarketplace(),
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    repository.failure = const ModulesException('Removal failed.');
    expect(await controller.remove(testModule.id), isFalse);
    expect(controller.errorMessage, 'Removal failed.');
    expect(controller.modules, hasLength(1));
    repository.failure = null;
    expect(await controller.remove(testModule.id), isTrue);
    expect(controller.modules, isEmpty);
    expect(controller.errorMessage, isNull);
  });

  test('a failure gives a reason only when its cause is known', () async {
    final repository = MemoryModulesRepository()..modules = [testModule];
    final controller = ModulesController(
      repository: repository,
      marketplace: MemoryModuleMarketplace()
        ..failure = const ModuleMarketplaceUnavailableException('Down.'),
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await controller.loadMarketplace();
    expect(controller.catalogState, ModulesLoadState.error);
    expect(controller.catalogError, isNull);

    repository.failure = StateError('unexpected');
    expect(await controller.remove(testModule.id), isFalse);
    expect(controller.errorMessage, isNotNull);
    expect(controller.errorReason, isNull);
    repository.failure = const ModulesException('Removal failed.');
    expect(await controller.remove(testModule.id), isFalse);
    expect(controller.errorReason, 'Removal failed.');
  });

  test(
    'offers newer versions and rejects equal versions or downgrades',
    () async {
      final repository = MemoryModulesRepository()..modules = [testModule];
      final newer = MarketplaceModule(
        id: testModule.id,
        title: testModule.title,
        description: testModule.description,
        version: Version.parse('1.1.0'),
        artifact: testMarketplaceModule.artifact,
      );
      final marketplace = MemoryModuleMarketplace();
      final controller = ModulesController(
        repository: repository,
        marketplace: marketplace,
      );
      addTearDown(controller.dispose);
      await controller.initialize();
      await controller.loadMarketplace();
      expect(controller.updateFor(testModule), isNull);
      await controller.install(testMarketplaceModule);
      expect(repository.calls, isEmpty);
      marketplace.modules = [newer];
      await controller.loadMarketplace();
      expect(controller.updateFor(testModule), newer);
      await controller.install(newer);
      expect(repository.calls, ['install']);
      expect(marketplace.disposedPackages, 1);
    },
  );

  test(
    'download progress is visible and concurrent install admission is rejected',
    () async {
      final repository = _PendingRepository();
      final marketplace = MemoryModuleMarketplace();
      final controller = ModulesController(
        repository: repository,
        marketplace: marketplace,
      );
      addTearDown(controller.dispose);
      await controller.initialize();
      await controller.loadMarketplace();
      final progress = <double?>[];
      controller.addListener(
        () => progress.add(controller.operationFor(testModule.id)?.progress),
      );
      final install = controller.install(testMarketplaceModule);
      await Future<void>.delayed(Duration.zero);
      expect(controller.activeModuleId, testModule.id);
      final operation = controller.operationFor(testModule.id)!;
      expect(operation.kind, ModuleOperationKind.install);
      expect(operation.phase, ModuleOperationPhase.install);
      expect(operation.progress, isNull);
      expect(controller.busy, isTrue);
      await controller.install(testMarketplaceModule);
      expect(marketplace.disposedPackages, 0);
      repository.pending.complete();
      await install;
      expect(progress, containsAll([0.5, 1.0]));
      expect(controller.activeModuleId, isNull);
      expect(controller.operationFor(testModule.id), isNull);
      expect(controller.busy, isFalse);
      expect(marketplace.disposedPackages, 1);
    },
  );

  test('a failed install or update keeps the phase that failed', () async {
    final repository = MemoryModulesRepository();
    final marketplace = MemoryModuleMarketplace()
      ..downloadFailure = const ModulesException('Download failed.');
    final controller = ModulesController(
      repository: repository,
      marketplace: marketplace,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await controller.loadMarketplace();
    await controller.install(testMarketplaceModule);
    var failure = controller.operationFor(testModule.id)!;
    expect(failure.failed, isTrue);
    expect(failure.kind, ModuleOperationKind.install);
    expect(failure.phase, ModuleOperationPhase.download);
    expect(controller.activeModuleId, isNull);
    expect(controller.busy, isFalse);

    marketplace.downloadFailure = null;
    await controller.install(testMarketplaceModule);
    expect(controller.operationFor(testModule.id), isNull);
    expect(controller.modules.single.id, testModule.id);

    marketplace.modules = [
      MarketplaceModule(
        id: testModule.id,
        title: testModule.title,
        description: testModule.description,
        version: Version.parse('1.1.0'),
        artifact: testMarketplaceModule.artifact,
      ),
    ];
    await controller.loadMarketplace();
    repository.failure = const ModulesException('Invalid signature.');
    await controller.install(marketplace.modules.single);
    failure = controller.operationFor(testModule.id)!;
    expect(failure.failed, isTrue);
    expect(failure.kind, ModuleOperationKind.update);
    expect(failure.phase, ModuleOperationPhase.install);
    expect(controller.errorMessage, 'Invalid signature.');

    repository.failure = null;
    await controller.setEnabled(testModule.id, false);
    expect(controller.operationFor(testModule.id), isNull);
  });

  test('a refresh failure after activation is not a failed phase', () async {
    final repository = _UnlistableAfterInstallRepository();
    final controller = ModulesController(
      repository: repository,
      marketplace: MemoryModuleMarketplace(),
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await controller.loadMarketplace();
    await controller.install(testMarketplaceModule);
    expect(controller.errorMessage, 'List failed.');
    expect(controller.operationFor(testModule.id), isNull);
  });

  test(
    'disposing during download cleans staging and prevents installation',
    () async {
      final repository = MemoryModulesRepository();
      final marketplace = _PendingMarketplace();
      final controller = ModulesController(
        repository: repository,
        marketplace: marketplace,
      );
      await controller.initialize();
      await controller.loadMarketplace();
      final install = controller.install(testMarketplaceModule);
      await Future<void>.delayed(Duration.zero);
      controller.dispose();
      marketplace.pending.complete(
        SelectedModulePackage(
          path: '/private/test.cpmodule',
          dispose: () async {
            marketplace.disposedPackages++;
          },
        ),
      );
      await install;
      expect(repository.calls, isEmpty);
      expect(marketplace.disposedPackages, 1);
    },
  );

  test(
    'install, disable, preferences, invoke, and remove share repository state',
    () async {
      final repository = MemoryModulesRepository();
      final marketplace = MemoryModuleMarketplace();
      final controller = ModulesController(
        repository: repository,
        marketplace: marketplace,
      );
      await controller.initialize();
      expect(controller.state, ModulesLoadState.ready);
      await controller.loadMarketplace();
      await controller.install(testMarketplaceModule);
      expect(controller.modules.single.id, testModule.id);
      expect(marketplace.disposedPackages, 1);
      await controller.setEnabled(testModule.id, false);
      expect(controller.modules.single.enabled, false);
      await controller.setPreferences(testModule.id, {'uppercase': true});
      expect(repository.lastPreferences, {'uppercase': true});
      final result = await controller.invoke(testModule.id, 'transform', {
        'text': 'Україна',
      });
      expect(result!.text, 'УКРАЇНА');
      await controller.remove(testModule.id);
      expect(controller.modules, isEmpty);
      controller.dispose();
    },
  );

  test(
    'failed download has no install side effect and failed installation releases staging',
    () async {
      final repository = MemoryModulesRepository();
      final marketplace = MemoryModuleMarketplace()
        ..downloadFailure = const ModulesException('Download failed.');
      final controller = ModulesController(
        repository: repository,
        marketplace: marketplace,
      );
      await controller.initialize();
      await controller.loadMarketplace();
      await controller.install(testMarketplaceModule);
      expect(repository.calls, isEmpty);
      marketplace.downloadFailure = null;
      repository.failure = const ModulesException('Invalid signature.');
      await controller.install(testMarketplaceModule);
      expect(controller.errorMessage, 'Invalid signature.');
      expect(marketplace.disposedPackages, 1);
      expect(controller.busy, false);
      expect(controller.modules, isEmpty);
      controller.dispose();
    },
  );

  test(
    'disposal while installation is pending does not notify disposed state',
    () async {
      final repository = _PendingRepository();
      final marketplace = MemoryModuleMarketplace();
      final controller = ModulesController(
        repository: repository,
        marketplace: marketplace,
      );
      await controller.initialize();
      await controller.loadMarketplace();
      final operation = controller.install(testMarketplaceModule);
      await Future<void>.delayed(Duration.zero);
      controller.dispose();
      repository.pending.complete();
      await operation;
      expect(marketplace.disposedPackages, 1);
      expect(marketplace.disposed, isTrue);
    },
  );

  test('reports the locale before listing and again on reconnect', () async {
    final repository = _ListingRepository()..modules = [testModule];
    final reconnects = StreamController<void>();
    final controller = ModulesController(
      repository: repository,
      marketplace: MemoryModuleMarketplace(),
      reconnects: reconnects.stream,
    );
    addTearDown(controller.dispose);
    controller.useLocale('uk');
    await controller.initialize();
    expect(repository.calls, ['locale:uk', 'list']);

    reconnects.add(null);
    await pumpEventQueue();
    expect(repository.calls, ['locale:uk', 'list', 'locale:uk', 'list']);
    await reconnects.close();
  });

  test('initialization does not wait for a search model download', () async {
    final repository = _PendingPreferencesRepository()
      ..modules = [
        const InstalledModule(
          id: 'copypaste.search',
          title: 'Search',
          description: 'Search history.',
          version: '1.0.0',
          enabled: true,
          sizeBytes: 10,
          commands: [],
          searchLanguageField: 'languages',
          searchModels: [
            ModuleSearchModel(
              id: 'en',
              title: 'English',
              languages: ['en'],
              sizeBytes: 10,
              available: false,
            ),
          ],
          preferenceFields: [
            ModuleField(
              id: 'languages',
              title: 'Languages',
              kind: ModuleFieldKind.choices,
              defaultValue: <String>[],
              options: [ModuleChoice(id: 'en', title: 'English')],
            ),
          ],
          preferences: {},
        ),
      ];
    final controller = ModulesController(
      repository: repository,
      marketplace: MemoryModuleMarketplace(),
      preferredLanguages: () => ['en'],
    );
    addTearDown(controller.dispose);

    await controller.initialize();

    expect(controller.state, ModulesLoadState.ready);
    expect(controller.configuringModel, isTrue);
    repository.pending.complete();
    await pumpEventQueue();
    expect(repository.lastPreferences, {
      'languages': ['en'],
    });
    expect(controller.busy, isFalse);
  });

  test('relists when the language resolves during the first list', () async {
    final repository = _ListingRepository()..modules = [testModule];
    final controller = ModulesController(
      repository: repository,
      marketplace: MemoryModuleMarketplace(),
    );
    addTearDown(controller.dispose);
    final initialized = controller.initialize();
    controller.useLocale('uk');
    await initialized;
    await pumpEventQueue();
    expect(repository.calls, ['locale:uk', 'list', 'locale:uk', 'list']);
  });
}

class _ListingRepository extends MemoryModulesRepository {
  @override
  Future<List<InstalledModule>> list() {
    calls.add('list');
    return super.list();
  }
}

class _UnlistableAfterInstallRepository extends MemoryModulesRepository {
  @override
  Future<List<InstalledModule>> list() async {
    if (calls.contains('install')) throw const ModulesException('List failed.');
    return super.list();
  }
}

class _PendingPreferencesRepository extends MemoryModulesRepository {
  final pending = Completer<void>();
  @override
  Future<void> setPreferences(String id, Map<String, Object> values) async {
    await pending.future;
    await super.setPreferences(id, values);
  }
}

class _PendingRepository extends MemoryModulesRepository {
  final pending = Completer<void>();
  @override
  Future<void> install(String packagePath) => pending.future;
}

class _PendingMarketplace extends MemoryModuleMarketplace {
  final pending = Completer<SelectedModulePackage>();
  @override
  Future<SelectedModulePackage> download(
    MarketplaceModule module, {
    required void Function(double) onProgress,
  }) => pending.future;
}
