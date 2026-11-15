import '../../../platform/clipboard/clipboard_writer.dart';
import '../../../platform/security/device_authenticator.dart';
import '../../applications/repository/application_catalog.dart';
import '../repository/module_text_delivery.dart';
import '../repository/modules_repository.dart';

/// What a module view needs from the app beyond the module itself.
class ModuleViewServices {
  const ModuleViewServices({
    this.authenticator,
    this.fileSaver,
    this.delivery,
    this.clipboard = const SystemClipboardWriter(),
    this.applications = const RuntimeApplicationCatalog(),
  });

  /// Verifies the device owner for `require_auth` actions and deliveries.
  final DeviceAuthenticator? authenticator;
  final ModuleFileSaver? fileSaver;

  /// Copies the text of a delivery; deliveries are ignored without it.
  final ModuleTextDelivery? delivery;
  final ClipboardWriter clipboard;
  final ApplicationCatalog applications;
}
