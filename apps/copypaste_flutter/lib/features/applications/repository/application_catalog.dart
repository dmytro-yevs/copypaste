import '../../../generated/api.dart' as runtime;
import '../../../platform/applications/installed_applications.dart';
import '../../history/models/history_models.dart';
import '../models/application_models.dart';

/// Where application choices come from: apps installed on this device and
/// source apps already seen in History.
abstract interface class ApplicationCatalog {
  Future<List<ApplicationChoice>> installed();

  Future<ApplicationChoice?> installedById(String id);

  Future<List<ApplicationChoice>> seen();

  Future<HistorySourceAppIcon?> installedIcon(String id);

  Future<HistorySourceAppIcon?> seenIcon(String iconId);

  /// Drops icon renders still waiting, such as rows of a closed picker.
  void cancelIcons();
}

class RuntimeApplicationCatalog implements ApplicationCatalog {
  const RuntimeApplicationCatalog([this._installed]);

  final InstalledApplications? _installed;

  InstalledApplications get _apps =>
      _installed ?? InstalledApplications.forPlatform();

  @override
  Future<List<ApplicationChoice>> installed() async => [
    for (final app in await _apps.list())
      ApplicationChoice(id: app.id, name: app.name),
  ];

  @override
  Future<ApplicationChoice?> installedById(String id) async {
    final app = await _apps.find(id);
    return app == null ? null : ApplicationChoice(id: app.id, name: app.name);
  }

  @override
  Future<List<ApplicationChoice>> seen() async => [
    for (final app in (await runtime.historyFacets()).sourceApps)
      ApplicationChoice(id: app.id, name: app.label, iconId: app.iconId),
  ];

  @override
  Future<HistorySourceAppIcon?> installedIcon(String id) => _apps.icon(id);

  @override
  void cancelIcons() => _apps.cancelIcons();

  @override
  Future<HistorySourceAppIcon?> seenIcon(String iconId) async {
    final preview = await runtime.sourceAppIcon(id: iconId);
    return preview == null
        ? null
        : HistorySourceAppIcon(
            preview.pngBytes,
            width: preview.width,
            height: preview.height,
          );
  }
}
