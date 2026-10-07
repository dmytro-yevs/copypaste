import '../../../platform/android/android_shizuku_state.dart';

/// Platform access is separate from signed module installation and execution.
class SmsModuleAccessState {
  const SmsModuleAccessState({
    required this.smsGranted,
    required this.notificationGranted,
    required this.shizuku,
    required this.adbCommands,
  });
  final bool smsGranted;
  final bool notificationGranted;
  final AndroidShizukuState shizuku;
  final String adbCommands;

  bool get granted => smsGranted && notificationGranted;
}

abstract interface class ModuleAccessRepository {
  Future<SmsModuleAccessState> smsState();
  Future<SmsModuleAccessState> configureSms();
  Future<SmsModuleAccessState> requestSmsNotifications();
  Future<bool> openShizuku();
  Future<bool> synchronize();
}
