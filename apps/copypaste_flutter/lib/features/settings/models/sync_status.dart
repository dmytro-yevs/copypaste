import 'package:copypaste_flutter/l10n/l10n.dart';

import '../../devices/devices_gateway.dart';

enum SyncPhase { unavailable, disabled, waiting, syncing, synced, failed }

enum SyncTone { muted, info, success, warning, error }

extension SyncPhaseLabel on SyncPhase {
  String get label {
    final strings = AppStrings.current;
    return switch (this) {
      SyncPhase.unavailable => strings.syncStatusUnavailable,
      SyncPhase.disabled => strings.syncStatusOff,
      SyncPhase.waiting => strings.syncStatusWaitingForSync,
      SyncPhase.syncing => strings.syncStatusSyncing,
      SyncPhase.synced => strings.syncStatusUpToDate,
      SyncPhase.failed => strings.syncStatusInterrupted,
    };
  }
}

/// The backend owns classification and recovery. UI never parses error text.
class SyncIssue {
  const SyncIssue({
    required this.code,
    this.scope = 'unknown',
    required this.title,
    required this.message,
    required this.severity,
    required this.blocking,
    required this.pending,
    required this.since,
    this.count = 0,
  });
  final String code;
  final String scope;
  bool get transportRelated => scope == 'connection';
  final String title;
  final String message;
  final String severity;
  final bool blocking;
  final bool pending;
  final DateTime since;
  final int count;
  SyncTone get tone => switch (severity) {
    'error' => SyncTone.error,
    'warning' => SyncTone.warning,
    _ => SyncTone.info,
  };
  @override
  bool operator ==(Object other) =>
      other is SyncIssue &&
      code == other.code &&
      scope == other.scope &&
      title == other.title &&
      message == other.message &&
      severity == other.severity &&
      blocking == other.blocking &&
      pending == other.pending &&
      since == other.since &&
      count == other.count;
  @override
  int get hashCode => Object.hash(
    code,
    title,
    message,
    severity,
    blocking,
    pending,
    since,
    count,
  );
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
    this.issues = const [],
    DeviceConnectionState connection = DeviceConnectionState.unknown,
    bool offline = false,
    this.checking = false,
    this.addressKnown = true,
  }) : connection = offline ? DeviceConnectionState.offline : connection;
  final String id;
  final String name;
  final SyncPhase phase;
  final DateTime? startedAt;
  final DateTime? lastSuccess;
  final int sent;
  final int received;
  final int skippedTooLarge;

  /// Backward-compatible diagnostic text; it is never used as a condition code.
  final String? error;
  final List<SyncIssue> issues;
  final DeviceConnectionState connection;
  final bool checking;
  final bool addressKnown;
  bool get offline => connection == DeviceConnectionState.offline;
  SyncIssue? get blocker => issues.where((issue) => issue.blocking).firstOrNull;
  bool get legacyProblem => issues.isEmpty && error != null;
  bool get hasPending =>
      skippedTooLarge > 0 || issues.any((issue) => issue.pending);
  bool get hasWarning =>
      issues.any((issue) => issue.pending && issue.severity == 'warning') ||
      skippedTooLarge > 0;
  bool get retrying => issues.any(
    (issue) =>
        !issue.blocking &&
        !issue.pending &&
        (!offline || !issue.transportRelated),
  );
  bool get upToDate =>
      connection == DeviceConnectionState.online &&
      phase == SyncPhase.synced &&
      issues.isEmpty &&
      skippedTooLarge == 0 &&
      error == null;
  String get connectionLabel {
    final strings = AppStrings.current;
    return switch (connection) {
      DeviceConnectionState.online => strings.devicesConnectionOnline,
      DeviceConnectionState.offline => strings.devicesConnectionOffline,
      _ =>
        checking
            ? strings.syncConnectionChecking
            : strings.syncConnectionUnknown,
    };
  }

  String get label {
    final strings = AppStrings.current;
    if (blocker case final issue?) return issue.title;
    if (phase == SyncPhase.unavailable || phase == SyncPhase.disabled) {
      return phase.label;
    }
    if (offline) return strings.devicesConnectionOffline;
    if (phase == SyncPhase.syncing) return strings.syncStatusSyncing;
    if (hasWarning || (phase == SyncPhase.synced && legacyProblem)) {
      return strings.syncStatusPartiallySynchronized;
    }
    if (retrying || phase == SyncPhase.failed) {
      return strings.syncStatusRetrying;
    }
    if (hasPending) return strings.syncStatusWaitingToSync;
    if (upToDate) return strings.syncStatusUpToDate;
    if (!addressKnown) return strings.syncStatusWaitingForDevice;
    if (connection != DeviceConnectionState.online) return connectionLabel;
    return strings.syncStatusWaitingForSync;
  }

  SyncTone get tone {
    if (blocker != null) return SyncTone.error;
    if (phase == SyncPhase.disabled ||
        phase == SyncPhase.unavailable ||
        offline) {
      return offline &&
              (visibleIssues.any((issue) => issue.severity == 'warning') ||
                  legacyProblem)
          ? SyncTone.warning
          : SyncTone.muted;
    }
    if (phase == SyncPhase.syncing) return SyncTone.info;
    if (hasWarning ||
        retrying ||
        phase == SyncPhase.failed ||
        (phase == SyncPhase.synced && legacyProblem)) {
      return SyncTone.warning;
    }
    return upToDate ? SyncTone.success : SyncTone.muted;
  }

  /// Transport interruptions are not actionable while offline. Confirmed
  /// blockers and item conditions remain visible regardless of connectivity.
  List<SyncIssue> get visibleIssues => [
    for (final issue in issues)
      if (issue.severity != 'info' && (!offline || !issue.transportRelated))
        issue,
  ];
}

class SyncLogEntry {
  const SyncLogEntry({
    required this.peerId,
    required this.timestamp,
    required this.kind,
    this.error,
    this.sent = 0,
    this.received = 0,
    this.issueTitle,
    this.issueSeverity,
  });
  final String peerId;
  final DateTime timestamp;
  final String kind;
  final String? error;
  final int sent;
  final int received;
  final String? issueTitle;
  final String? issueSeverity;
  @override
  bool operator ==(Object other) =>
      other is SyncLogEntry &&
      peerId == other.peerId &&
      timestamp == other.timestamp &&
      kind == other.kind &&
      error == other.error &&
      sent == other.sent &&
      received == other.received &&
      issueTitle == other.issueTitle &&
      issueSeverity == other.issueSeverity;
  @override
  int get hashCode => Object.hash(
    peerId,
    timestamp,
    kind,
    error,
    sent,
    received,
    issueTitle,
    issueSeverity,
  );
  String get message {
    final strings = AppStrings.current;
    return switch (kind) {
      'offline' => strings.syncLogDeviceOffline,
      'online' => strings.syncLogConnectionRestored,
      'problem_detected' => issueTitle ?? strings.syncLogProblemDetected,
      'problem_resolved' => error ?? strings.syncLogProblemResolved,
      'failed' => error ?? strings.syncLogFailed,
      'recovered' => strings.syncLogRecovered,
      _ => strings.syncLogClipsTransferred(sent, received),
    };
  }
}

class SyncStatus {
  const SyncStatus({
    this.revision = 0,
    this.phase = SyncPhase.unavailable,
    this.peers = const [],
    this.log = const [],
    this.diagnosticsAvailable = true,
  });
  final int revision;
  final SyncPhase phase;
  final List<PeerSyncStatus> peers;
  final List<SyncLogEntry> log;
  final bool diagnosticsAvailable;
  bool get hasPendingItems => peers.any(
    (peer) =>
        peer.hasPending ||
        (peer.phase == SyncPhase.synced && peer.legacyProblem),
  );
  int get _blocked => peers.where((peer) => peer.blocker != null).length;
  String get label => diagnosticsAvailable
      ? _label
      : AppStrings.current.syncStatusWithActivityLogUnavailable(_label);
  String get _label {
    final strings = AppStrings.current;
    if (phase == SyncPhase.unavailable) return phase.label;
    if (phase == SyncPhase.disabled) {
      return _blocked > 0
          ? strings.syncStatusOffWithIssues(_blocked)
          : strings.syncStatusOff;
    }
    if (peers.isEmpty) return strings.syncStatusNoPairedDevices;
    final offline = peers.where((peer) => peer.offline).length;
    String withOffline(String status) => offline > 0 && offline != peers.length
        ? strings.syncStatusWithOfflineDevices(status, offline)
        : status;
    if (_blocked > 0) {
      return withOffline(strings.syncStatusDevicesNeedAttention(_blocked));
    }
    if (peers.every((peer) => peer.offline)) {
      final warnings = peers
          .where((peer) => peer.tone == SyncTone.warning)
          .length;
      return warnings > 0
          ? strings.syncStatusDevicesOfflineWithIssues(warnings)
          : strings.syncStatusDevicesOffline;
    }
    if (peers.any((peer) => peer.phase == SyncPhase.syncing && !peer.offline)) {
      return withOffline(strings.syncStatusSyncing);
    }
    if (peers.any(
      (peer) =>
          !peer.offline &&
          (peer.hasWarning ||
              (peer.phase == SyncPhase.synced && peer.legacyProblem)),
    )) {
      return withOffline(strings.syncStatusPartiallySynchronized);
    }
    if (peers.any(
      (peer) =>
          peer.retrying || (!peer.offline && peer.phase == SyncPhase.failed),
    )) {
      return withOffline(strings.syncStatusRetrying);
    }
    if (peers.every((peer) => peer.upToDate || peer.offline)) {
      return withOffline(strings.syncStatusUpToDate);
    }
    return withOffline(strings.syncStatusWaitingForSync);
  }

  SyncTone get tone {
    if (_blocked > 0) return SyncTone.error;
    if (!diagnosticsAvailable) return SyncTone.warning;
    if (phase == SyncPhase.unavailable || phase == SyncPhase.disabled) {
      return SyncTone.muted;
    }
    if (peers.any((peer) => peer.tone == SyncTone.info)) return SyncTone.info;
    if (peers.any((peer) => peer.tone == SyncTone.warning)) {
      return SyncTone.warning;
    }
    return peers.any((peer) => peer.upToDate) &&
            peers.every((peer) => peer.upToDate || peer.offline)
        ? SyncTone.success
        : SyncTone.muted;
  }
}
