import '../../history/models/history_models.dart';
import '../models/ai_tools_models.dart';

/// Runtime access for AI tool settings, approvals and item visibility.
///
/// Failures are [AiToolsException]s carrying the runtime error code.
/// Administrative calls are served by the daemon only to this app.
abstract interface class AiToolsRepository {
  /// Whether this desktop serves AI tools; a device-local setting.
  Future<bool> enabled();

  Future<bool> setEnabled(bool enabled);

  Future<AiToolsStatus> status();

  Future<AiToolsPolicy> policy();

  /// Sets one synced policy value ([AiToolsPolicyKey]).
  Future<AiToolsPolicy> setPolicy(String key, Object value);

  /// Adds or removes one member of a synced policy set ([AiToolsPolicySet]).
  Future<AiToolsPolicy> setPolicyMember(
    String set,
    String member, {
    required bool present,
  });

  /// Devices and source applications the policy editor offers.
  Future<AiToolsExposureOptions> exposureOptions();

  Future<HistorySourceAppIcon?> sourceAppIcon(String iconId);

  /// The stable id of this device, which tells local grants apart.
  Future<String?> localDeviceId();

  /// Grants of every desktop, local ones first.
  Future<List<AiToolGrant>> grants();

  Future<AiToolGrantCreated> createGrant({
    required String name,
    required AiToolClient client,
    required Set<AiToolScope> scopes,
    required AiToolLookback lookback,
  });

  Future<AiToolGrant> updateGrant(
    String id, {
    String? name,
    Set<AiToolScope>? scopes,
    AiToolLookback? lookback,
  });

  Future<AiToolGrant> revokeGrant(String id);

  Future<void> deleteGrant(String id);

  Future<List<AiAccessEntry>> accessLog(String grantId);

  Future<List<AiApprovalRequest>> pendingApprovals();

  Future<void> approve(
    String requestId, {
    required Set<AiToolScope> scopes,
    required AiToolLookback lookback,
  });

  Future<void> deny(String requestId);

  Future<void> decideVolume(String requestId, {required bool allow});

  Future<void> setHiddenFromAi(List<String> clipIds, {required bool hidden});

  /// Puts [text] on the clipboard as a concealed, transient write that
  /// creates no history row; answers the restore token, if any.
  Future<String?> copySecret(String text);

  /// Puts the previous clipboard back after [after], unless it changed.
  Future<void> restoreClipboard(String token, Duration after);

  /// Signals that grants or the policy changed, here or on a peer.
  Stream<void> changes();

  /// Signals that approval requests opened or resolved, and reconnects.
  Stream<void> approvalChanges();
}

/// Policy preference keys the runtime accepts.
abstract final class AiToolsPolicyKey {
  static const includeOtherDevices = 'include_other_devices';
  static const contentKinds = 'content_kinds';
  static const defaultLookback = 'default_lookback';
}

/// Policy sets the runtime accepts.
abstract final class AiToolsPolicySet {
  static const excludedDevices = 'excluded_devices';
  static const excludedApps = 'excluded_apps';
}
