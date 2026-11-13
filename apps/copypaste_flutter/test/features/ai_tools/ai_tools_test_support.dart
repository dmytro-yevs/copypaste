import 'dart:async';

import 'package:copypaste_flutter/features/ai_tools/models/ai_tools_models.dart';
import 'package:copypaste_flutter/features/ai_tools/repository/ai_approval_notifier.dart';
import 'package:copypaste_flutter/features/ai_tools/repository/ai_tools_repository.dart';
import 'package:copypaste_flutter/features/history/models/history_models.dart';

const cliPath = '/Applications/CopyPaste.app/Contents/MacOS/copypaste-cli';

AiToolGrant grantOf(
  String id, {
  String owner = 'desktop-1',
  String ownerName = 'Studio Mac',
}) => AiToolGrant(
  id: id,
  ownerDeviceId: owner,
  ownerDeviceName: ownerName,
  name: 'Claude Desktop',
  clientName: 'Claude Desktop',
  scopes: defaultAiToolScopes,
  lookback: AiToolLookback.week,
  createdAt: DateTime(2026, 11),
);

/// Records every runtime call and answers from in-memory state.
class FakeAiToolsRepository implements AiToolsRepository {
  final changeEvents = StreamController<void>.broadcast();
  final approvalEvents = StreamController<void>.broadcast();
  final copied = <String>[];
  final restored = <(String, Duration)>[];
  final denied = <String>[];
  final calls = <String>[];
  var policyValue = const AiToolsPolicy();
  var grantList = <AiToolGrant>[];
  var pending = <AiApprovalRequest>[];
  var serving = false;
  String deviceId = 'desktop-1';

  @override
  Future<bool> enabled() async => serving;

  @override
  Future<bool> setEnabled(bool enabled) async => serving = enabled;

  @override
  Future<AiToolsStatus> status() async =>
      AiToolsStatus(enabled: serving, contributingDevices: 2);

  @override
  Future<AiToolsPolicy> policy() async => policyValue;

  @override
  Future<AiToolsPolicy> setPolicy(String key, Object value) async {
    calls.add('setPolicy:$key');
    return policyValue;
  }

  @override
  Future<AiToolsPolicy> setPolicyMember(
    String set,
    String member, {
    required bool present,
  }) async {
    calls.add('setPolicyMember:$set');
    return policyValue;
  }

  @override
  Future<AiToolsExposureOptions> exposureOptions() async =>
      const AiToolsExposureOptions(
        devices: [AiToolsOption(id: 'phone-1', label: 'Pixel')],
      );

  @override
  Future<HistorySourceAppIcon?> sourceAppIcon(String iconId) async => null;

  @override
  Future<String?> localDeviceId() async => deviceId;

  @override
  Future<List<AiToolGrant>> grants() async => grantList;

  @override
  Future<AiToolGrantCreated> createGrant({
    required String name,
    required AiToolClient client,
    required Set<AiToolScope> scopes,
    required AiToolLookback lookback,
  }) async {
    final grant = grantOf('created', owner: deviceId);
    return AiToolGrantCreated(
      grant: grant,
      configs: [
        AiClientConfig(
          client: client.wireName,
          label: client.brandName ?? 'Other',
          language: 'json',
          text:
              '{"mcpServers": {"copypaste": {"command": "$cliPath", '
              '"args": ["mcp"], "env": {"COPYPASTE_MCP_GRANT": "cpmcp_x"}}}}',
        ),
      ],
    );
  }

  @override
  Future<AiToolGrant> updateGrant(
    String id, {
    String? name,
    Set<AiToolScope>? scopes,
    AiToolLookback? lookback,
  }) async => grantList.firstWhere((grant) => grant.id == id);

  @override
  Future<AiToolGrant> revokeGrant(String id) async =>
      grantList.firstWhere((grant) => grant.id == id);

  @override
  Future<void> deleteGrant(String id) async {}

  @override
  Future<List<AiAccessEntry>> accessLog(String grantId) async => const [];

  @override
  Future<List<AiApprovalRequest>> pendingApprovals() async => pending;

  @override
  Future<void> approve(
    String requestId, {
    required Set<AiToolScope> scopes,
    required AiToolLookback lookback,
  }) async => pending.removeWhere((request) => request.requestId == requestId);

  @override
  Future<void> deny(String requestId) async {
    denied.add(requestId);
    pending.removeWhere((request) => request.requestId == requestId);
  }

  @override
  Future<void> decideVolume(String requestId, {required bool allow}) async =>
      pending.removeWhere((request) => request.requestId == requestId);

  @override
  Future<void> setHiddenFromAi(
    List<String> clipIds, {
    required bool hidden,
  }) async {}

  @override
  Future<String?> copySecret(String text) async {
    copied.add(text);
    return 'restore-1';
  }

  @override
  Future<void> restoreClipboard(String token, Duration after) async =>
      restored.add((token, after));

  @override
  Stream<void> changes() => changeEvents.stream;

  @override
  Stream<void> approvalChanges() => approvalEvents.stream;
}

class FakeAiApprovalNotifier implements AiApprovalNotifier {
  final openedEvents = StreamController<String>.broadcast();
  final shown = <AiApprovalRequest>[];
  var dismissed = 0;

  @override
  Stream<String> get opened => openedEvents.stream;

  @override
  Future<void> show(AiApprovalRequest request) async => shown.add(request);

  @override
  Future<void> dismiss() async => dismissed++;
}
