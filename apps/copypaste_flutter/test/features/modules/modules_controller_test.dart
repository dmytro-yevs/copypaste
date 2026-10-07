import 'dart:async';
import 'package:pub_semver/pub_semver.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:copypaste_flutter/features/modules/controller/modules_controller.dart';
import 'package:copypaste_flutter/features/modules/models/module_models.dart';
import 'package:copypaste_flutter/features/modules/models/module_marketplace_models.dart';
import 'package:copypaste_flutter/features/modules/repository/modules_repository.dart';
import 'modules_test_support.dart';

void main() {
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
      controller.addListener(() => progress.add(controller.downloadProgress));
      final install = controller.install(testMarketplaceModule);
      await Future<void>.delayed(Duration.zero);
      expect(controller.activeModuleId, testModule.id);
      expect(controller.installing, isTrue);
      expect(controller.busy, isTrue);
      await controller.install(testMarketplaceModule);
      expect(marketplace.disposedPackages, 0);
      repository.pending.complete();
      await install;
      expect(progress, containsAll([0.5, 1.0]));
      expect(controller.activeModuleId, isNull);
      expect(controller.busy, isFalse);
      expect(marketplace.disposedPackages, 1);
    },
  );

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
