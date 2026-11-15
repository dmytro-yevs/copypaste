import 'package:pub_semver/pub_semver.dart';

import '../repository/module_marketplace_repository.dart';
import '../repository/module_update_stager.dart';
import '../repository/modules_repository.dart';

/// Prepares installed modules for the destination app, independently of the
/// settings screen and its cached catalog. Module failures never block the app.
class ModuleAppUpdatePreparer {
  ModuleAppUpdatePreparer({
    required ModulesRepository installed,
    required ModuleMarketplaceRepository Function(Version) marketplace,
    required ModuleUpdateStager stager,
  }) : _installed = installed,
       _marketplace = marketplace,
       _stager = stager;

  final ModulesRepository _installed;
  final ModuleMarketplaceRepository Function(Version) _marketplace;
  final ModuleUpdateStager _stager;

  Future<void> prepare(Version appVersion) async {
    ModuleMarketplaceRepository? marketplace;
    try {
      final installed = await _installed.list();
      if (installed.isEmpty) return;
      marketplace = _marketplace(appVersion);
      final catalog = await marketplace.list();
      for (final module in installed) {
        if (module.restartRequired) continue;
        try {
          final release = catalog
              .where(
                (release) =>
                    release.id == module.id &&
                    release.canInstall &&
                    release.version > Version.parse(module.version),
              )
              .firstOrNull;
          if (release == null) continue;
          final package = await marketplace.download(
            release,
            onProgress: (_) {},
          );
          try {
            await _stager.stage(
              id: module.id,
              packagePath: package.path,
              appVersion: appVersion,
            );
          } finally {
            await package.dispose();
          }
        } catch (_) {
          // Preserve this module and continue preparing the remaining modules.
        }
      }
    } catch (_) {
      // Missing/offline catalogs leave every installed module unchanged.
    } finally {
      marketplace?.dispose();
    }
  }
}
