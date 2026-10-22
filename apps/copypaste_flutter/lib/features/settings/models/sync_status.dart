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

  /// Present confirmed offline peers as waiting, retaining session diagnostics
  /// in the original backend snapshot for when connectivity changes.
  SyncStatus withOfflinePeers(Set<String> offlinePeerIds) {
    if (offlinePeerIds.isEmpty) return this;
    if (!peers.any((peer) => offlinePeerIds.contains(peer.id))) return this;
    final visiblePeers = [
      for (final peer in peers)
        if (offlinePeerIds.contains(peer.id))
          PeerSyncStatus(
            id: peer.id,
            name: peer.name,
            phase: switch (phase) {
              SyncPhase.unavailable || SyncPhase.disabled => phase,
              _ => SyncPhase.waiting,
            },
            startedAt: peer.startedAt,
            lastSuccess: peer.lastSuccess,
            sent: peer.sent,
            received: peer.received,
            skippedTooLarge: peer.skippedTooLarge,
          )
        else
          peer,
    ];
    final visiblePhase = switch (phase) {
      SyncPhase.unavailable || SyncPhase.disabled => phase,
      _ when visiblePeers.any((peer) => peer.phase == SyncPhase.syncing) =>
        SyncPhase.syncing,
      _ when visiblePeers.any((peer) => peer.phase == SyncPhase.failed) =>
        SyncPhase.failed,
      _ => SyncPhase.waiting,
    };
    return SyncStatus(
      revision: revision,
      phase: visiblePhase,
      peers: visiblePeers,
    );
  }
}
