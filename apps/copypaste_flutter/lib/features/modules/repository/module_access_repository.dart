import '../../../platform/android/android_shizuku_state.dart';

/// Platform access is separate from signed module installation and execution.
class SmsModuleAccessState {
  const SmsModuleAccessState({
    required this.smsAppInstalled,
    required this.smsGranted,
    required this.notificationGranted,
    required this.shizuku,
    required this.adbCommands,
  });

  /// The separate CopyPaste SMS app, the only holder of SMS access.
  final bool smsAppInstalled;
  final bool smsGranted;
  final bool notificationGranted;
  final AndroidShizukuState shizuku;
  final String adbCommands;

  bool get granted => smsGranted && notificationGranted;
}

/// The outcome of applying the enabled SMS modules to native monitoring.
enum SmsMonitoringStatus {
  /// Monitoring matches the enabled SMS modules.
  ready,

  /// The runtime has not reported whether an SMS module is enabled yet.
  unavailable,

  /// An SMS module is enabled, but SMS access is not granted.
  accessMissing,

  /// The system refused to start the monitoring service.
  serviceRefused,
}

abstract interface class ModuleAccessRepository {
  Future<SmsModuleAccessState> smsState();

  /// Native setup changes, published only while a listener is attached.
  Stream<SmsModuleAccessState> get smsChanges;
  Future<SmsModuleAccessState> configureSms();
  Future<SmsModuleAccessState> requestSmsNotifications();
  Future<bool> openShizuku();
  Future<void> installSmsApp();
  Future<SmsMonitoringStatus> synchronize();
}
