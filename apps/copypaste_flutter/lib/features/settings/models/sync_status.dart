enum SyncPhase { unavailable, disabled, waiting, syncing, synced, failed }

extension SyncPhaseLabel on SyncPhase {
  String get label => switch (this) {
    SyncPhase.unavailable => 'Sync state unavailable',
    SyncPhase.disabled => 'Sync disabled',
    SyncPhase.waiting => 'Waiting for sync',
    SyncPhase.syncing => 'Syncing',
    SyncPhase.synced => 'Sync completed',
    SyncPhase.failed => 'Sync failed',
  };
}

class PeerSyncStatus {
  const PeerSyncStatus({
    required this.id,
    required this.name,
    required this.phase,
    this.startedAt,
    this.lastSuccess,
    this.sent = 0,
    this.received = 0,
    this.skippedTooLarge = 0,
    this.error,
  });

  final String id;
  final String name;
  final SyncPhase phase;
  final DateTime? startedAt;
  final DateTime? lastSuccess;
  final int sent;
  final int received;
  final int skippedTooLarge;
  final String? error;
}

class SyncStatus {
  const SyncStatus({
    this.revision = 0,
    this.phase = SyncPhase.unavailable,
    this.peers = const [],
  });

  final int revision;
  final SyncPhase phase;
  final List<PeerSyncStatus> peers;
}
