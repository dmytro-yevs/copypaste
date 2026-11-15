enum ModuleOperationKind { install, update }

/// Stages the host can observe. The runtime authenticates, extracts and
/// activates a package inside the single install request.
enum ModuleOperationPhase { download, install }

enum ModuleOperationStatus { running, failed }

/// Where one module install or update currently is.
class ModuleOperation {
  const ModuleOperation._({
    required this.moduleId,
    required this.kind,
    required this.phase,
    required this.status,
    required this.totalBytes,
    required double downloaded,
  }) : _downloaded = downloaded;

  const ModuleOperation.started({
    required String moduleId,
    required ModuleOperationKind kind,
    required int totalBytes,
  }) : this._(
         moduleId: moduleId,
         kind: kind,
         phase: ModuleOperationPhase.download,
         status: ModuleOperationStatus.running,
         totalBytes: totalBytes,
         downloaded: 0,
       );

  final String moduleId;
  final ModuleOperationKind kind;
  final ModuleOperationPhase phase;
  final ModuleOperationStatus status;
  final int totalBytes;
  final double _downloaded;

  bool get running => status == ModuleOperationStatus.running;
  bool get failed => status == ModuleOperationStatus.failed;

  /// Null while the current phase has no measurable progress.
  double? get progress =>
      running && phase == ModuleOperationPhase.download ? _downloaded : null;

  int get downloadPercent => (_downloaded * 100).floor();
  int get downloadedBytes => (_downloaded * totalBytes).round();

  /// Returns this instance until the whole percentage changes, so listeners
  /// are not notified for every received chunk.
  ModuleOperation downloadedTo(double fraction) {
    if (!running || phase != ModuleOperationPhase.download) return this;
    final bounded = fraction.clamp(0.0, 1.0);
    if ((bounded * 100).floor() == (_downloaded * 100).floor()) return this;
    return _copy(downloaded: bounded);
  }

  ModuleOperation installing() =>
      running ? _copy(phase: ModuleOperationPhase.install) : this;

  ModuleOperation failedInPhase() =>
      _copy(status: ModuleOperationStatus.failed);

  ModuleOperation _copy({
    ModuleOperationPhase? phase,
    ModuleOperationStatus? status,
    double? downloaded,
  }) => ModuleOperation._(
    moduleId: moduleId,
    kind: kind,
    phase: phase ?? this.phase,
    status: status ?? this.status,
    totalBytes: totalBytes,
    downloaded: downloaded ?? _downloaded,
  );
}
