import 'package:flutter_test/flutter_test.dart';
import 'package:pub_semver/pub_semver.dart';
import 'package:copypaste_flutter/features/modules/controller/module_app_update_preparer.dart';
import 'package:copypaste_flutter/features/modules/models/module_marketplace_models.dart';
import 'package:copypaste_flutter/features/modules/models/module_models.dart';
import 'package:copypaste_flutter/features/modules/repository/module_update_stager.dart';
import 'package:copypaste_flutter/features/modules/repository/modules_repository.dart';

import 'modules_test_support.dart';

void main() {
  test(
    'uses destination app compatibility and only stages installed updates',
    () async {
      final installed = MemoryModulesRepository()..modules = [testModule];
      final marketplace = _Marketplace()
        ..modules = [
          _release(testModule.id),
          _release('copypaste.not-installed'),
        ];
      final stager = _Stager();
      Version? requestedVersion;
      final preparer = ModuleAppUpdatePreparer(
        installed: installed,
        marketplace: (version) {
          requestedVersion = version;
          return marketplace;
        },
        stager: stager,
      );
      await preparer.prepare(Version.parse('2.0.0'));
      expect(requestedVersion, Version.parse('2.0.0'));
      expect(stager.ids, [testModule.id]);
      expect(stager.versions, [Version.parse('2.0.0')]);
      expect(marketplace.downloads, [testModule.id]);
      expect(marketplace.disposedPackages, 1);
      expect(marketplace.disposed, isTrue);
      expect(installed.calls, isEmpty);
    },
  );

  test(
    'missing, current and incompatible releases leave installed modules alone',
    () async {
      final marketplace = _Marketplace()
        ..modules = [
          _release('copypaste.current', version: '1.0.0'),
          _release('copypaste.incompatible', available: false),
          _release('copypaste.older', version: '0.9.0'),
        ];
      final stager = _Stager();
      await ModuleAppUpdatePreparer(
        installed: MemoryModulesRepository()
          ..modules = [
            _installed('copypaste.current'),
            _installed('copypaste.incompatible'),
            _installed('copypaste.missing'),
            _installed('copypaste.older'),
          ],
        marketplace: (_) => marketplace,
        stager: stager,
      ).prepare(Version.parse('2.0.0'));
      expect(marketplace.downloads, isEmpty);
      expect(stager.ids, isEmpty);
    },
  );

  test(
    'download and staging failures do not stop other module updates',
    () async {
      final marketplace = _Marketplace()
        ..modules = [
          for (final id in ['download', 'stage', 'ok'])
            _release('copypaste.$id'),
        ]
        ..failDownload = 'copypaste.download';
      final stager = _Stager()..failId = 'copypaste.stage';
      await ModuleAppUpdatePreparer(
        installed: MemoryModulesRepository()
          ..modules = [
            for (final id in ['download', 'stage', 'ok'])
              _installed('copypaste.$id'),
          ],
        marketplace: (_) => marketplace,
        stager: stager,
      ).prepare(Version.parse('2.0.0'));
      expect(stager.ids, ['copypaste.stage', 'copypaste.ok']);
      expect(marketplace.disposedPackages, 2);
      expect(marketplace.disposed, isTrue);
    },
  );

  test(
    'offline catalog is skipped without changing installed modules',
    () async {
      final marketplace = _Marketplace()..failure = Exception('Offline');
      final installed = MemoryModulesRepository()..modules = [testModule];
      final stager = _Stager();
      await ModuleAppUpdatePreparer(
        installed: installed,
        marketplace: (_) => marketplace,
        stager: stager,
      ).prepare(Version.parse('2.0.0'));
      expect(stager.ids, isEmpty);
      expect(installed.calls, isEmpty);
      expect(marketplace.disposed, isTrue);
    },
  );
}

MarketplaceModule _release(
  String id, {
  String version = '1.1.0',
  bool available = true,
}) => MarketplaceModule(
  id: id,
  title: id,
  description: '',
  version: Version.parse(version),
  artifact: testMarketplaceModule.artifact,
  availability: available
      ? ModuleAvailability.available
      : ModuleAvailability.appVersion,
);

InstalledModule _installed(String id) => InstalledModule(
  id: id,
  title: id,
  description: '',
  version: '1.0.0',
  enabled: false,
  sizeBytes: 0,
  commands: const [],
  preferenceFields: const [],
  preferences: const {},
);

class _Marketplace extends MemoryModuleMarketplace {
  final downloads = <String>[];
  String? failDownload;

  @override
  Future<SelectedModulePackage> download(
    MarketplaceModule module, {
    required void Function(double) onProgress,
  }) async {
    downloads.add(module.id);
    if (module.id == failDownload) throw Exception('Download failed');
    return super.download(module, onProgress: onProgress);
  }
}

class _Stager implements ModuleUpdateStager {
  final ids = <String>[];
  final versions = <Version>[];
  String? failId;

  @override
  Future<void> stage({
    required String id,
    required String packagePath,
    required Version appVersion,
  }) async {
    ids.add(id);
    versions.add(appVersion);
    if (id == failId) throw Exception('Stage failed');
  }
}
