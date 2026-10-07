class AndroidShizukuState {
  const AndroidShizukuState({
    required this.supported,
    required this.installed,
    required this.running,
    required this.permission,
  });

  final bool supported;
  final bool installed;
  final bool running;
  final bool permission;
}
