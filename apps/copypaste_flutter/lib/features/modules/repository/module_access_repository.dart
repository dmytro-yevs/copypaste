/// Platform access is separate from signed module installation and execution.
class SmsModuleAccessState {
  const SmsModuleAccessState({
    required this.granted,
    required this.adbCommands,
  });
  final bool granted;
  final String adbCommands;
}

abstract interface class ModuleAccessRepository {
  Future<SmsModuleAccessState> smsState();
  Future<SmsModuleAccessState> configureSms();
  Future<bool> synchronize();
}
