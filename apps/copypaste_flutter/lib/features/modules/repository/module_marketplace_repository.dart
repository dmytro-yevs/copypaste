import '../models/module_marketplace_models.dart';
import 'modules_repository.dart';

abstract interface class ModuleMarketplaceRepository {
  Future<List<MarketplaceModule>> list();
  Future<SelectedModulePackage> download(
    MarketplaceModule module, {
    required void Function(double progress) onProgress,
  });
  void dispose();
}
