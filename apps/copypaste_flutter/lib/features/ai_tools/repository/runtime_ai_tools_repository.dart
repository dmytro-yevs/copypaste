import 'dart:convert';

import 'package:copypaste_flutter/generated/api.dart' as runtime;
import 'package:copypaste_flutter/generated/api/ai.dart' as api;
import 'package:copypaste_flutter/generated/api/clips.dart' as clips;

import '../../../shared/runtime_event_service.dart';
import '../../history/models/history_models.dart';
import '../models/ai_tools_models.dart';
import 'ai_tools_repository.dart';

/// [AiToolsRepository] over the runtime bridge.
class RuntimeAiToolsRepository implements AiToolsRepository {
  RuntimeAiToolsRepository({RuntimeEventService? events})
    : _events = events ?? RuntimeEventService.instance;

  static const _dayMs = 24 * 60 * 60 * 1000;

  final RuntimeEventService _events;

  @override
  Future<bool> enabled() => _guard(api.aiToolsEnabled);

  @override
  Future<bool> setEnabled(bool enabled) =>
      _guard(() => api.setAiToolsEnabled(enabled: enabled));

  @override
  Future<AiToolsStatus> status() => _guard(() async {
    final status = await api.aiStatus();
    return AiToolsStatus(
      enabled: status.mcpEnabled,
      contributingDevices: status.contributingDevices,
    );
  });

  @override
  Future<AiToolsPolicy> policy() =>
      _guard(() async => _policy(await api.aiPolicy()));

  @override
  Future<AiToolsPolicy> setPolicy(String key, Object value) => _guard(
    () async => _policy(
      await api.setAiPolicy(key: key, valueJson: jsonEncode(_wire(value))),
    ),
  );

  @override
  Future<AiToolsPolicy> setPolicyMember(
    String set,
    String member, {
    required bool present,
  }) => _guard(
    () async => _policy(
      await api.setAiPolicyMember(set_: set, member: member, present: present),
    ),
  );

  @override
  Future<AiToolsExposureOptions> exposureOptions() => _guard(() async {
    final facets = await runtime.historyFacets();
    return AiToolsExposureOptions(
      devices: [
        for (final device in facets.originDevices)
          AiToolsOption(id: device.id, label: device.label),
      ],
      sourceApps: [
        for (final app in facets.sourceApps)
          AiToolsOption(id: app.id, label: app.label, iconId: app.iconId),
      ],
    );
  });

  @override
  Future<HistorySourceAppIcon?> sourceAppIcon(String iconId) =>
      _guard(() async {
        final icon = await runtime.sourceAppIcon(id: iconId);
        if (icon == null) return null;
        return HistorySourceAppIcon(
          icon.pngBytes,
          width: icon.width,
          height: icon.height,
        );
      });

  @override
  Future<String?> localDeviceId() =>
      _guard(() async => (await runtime.runtimeStatus()).deviceId);

  @override
  Future<List<AiToolGrant>> grants() => _guard(
    () async => [for (final grant in await api.aiGrants()) _grant(grant)],
  );

  @override
  Future<AiToolGrantCreated> createGrant({
    required String name,
    required AiToolClient client,
    required Set<AiToolScope> scopes,
    required AiToolLookback lookback,
  }) => _guard(() async {
    final created = await api.createAiGrant(
      name: name,
      client: client.wireName,
      scopes: _scopes(scopes),
      lookback: _lookback(lookback),
    );
    return AiToolGrantCreated(
      grant: _grant(created.grant),
      configs: [
        for (final snippet in created.snippets)
          AiClientConfig(
            client: snippet.client,
            label: snippet.label,
            language: snippet.language,
            text: snippet.text,
          ),
      ],
    );
  });

  @override
  Future<AiToolGrant> updateGrant(
    String id, {
    String? name,
    Set<AiToolScope>? scopes,
    AiToolLookback? lookback,
  }) => _guard(
    () async => _grant(
      await api.updateAiGrant(
        id: id,
        name: name,
        scopes: scopes == null ? null : _scopes(scopes),
        lookback: lookback == null ? null : _lookback(lookback),
      ),
    ),
  );

  @override
  Future<AiToolGrant> revokeGrant(String id) =>
      _guard(() async => _grant(await api.revokeAiGrant(id: id)));

  @override
  Future<void> deleteGrant(String id) =>
      _guard(() => api.deleteAiGrant(id: id));

  @override
  Future<List<AiAccessEntry>> accessLog(String grantId) => _guard(
    () async => [
      for (final entry in await api.aiAccessLog(grantId: grantId))
        AiAccessEntry(
          at: DateTime.fromMillisecondsSinceEpoch(entry.atMs),
          tool: entry.tool,
          resultCount: entry.resultCount,
          error: entry.error,
        ),
    ],
  );

  @override
  Future<List<AiApprovalRequest>> pendingApprovals() => _guard(
    () async => [
      for (final request in await api.aiPendingApprovals())
        AiApprovalRequest(
          requestId: request.requestId,
          kind: switch (request.kind) {
            api.AiApprovalKind.access => AiApprovalKind.access,
            api.AiApprovalKind.volume => AiApprovalKind.volume,
          },
          clientName: request.clientName,
          clientVersion: request.clientVersion,
          executable: request.executable,
          grantName: request.grantName,
          expiresAt: DateTime.fromMillisecondsSinceEpoch(request.expiresAtMs),
        ),
    ],
  );

  @override
  Future<void> approve(
    String requestId, {
    required Set<AiToolScope> scopes,
    required AiToolLookback lookback,
  }) => _guard(
    () => api.approveAiRequest(
      requestId: requestId,
      scopes: _scopes(scopes),
      lookback: _lookback(lookback),
    ),
  );

  @override
  Future<void> deny(String requestId) =>
      _guard(() => api.denyAiRequest(requestId: requestId));

  @override
  Future<void> decideVolume(String requestId, {required bool allow}) =>
      _guard(() => api.decideAiVolume(requestId: requestId, allow: allow));

  @override
  Future<void> setHiddenFromAi(List<String> clipIds, {required bool hidden}) =>
      _guard(() => clips.setClipsHiddenFromAi(ids: clipIds, hidden: hidden));

  @override
  Future<String?> copySecret(String text) => _guard(
    () async => (await clips.deliverText(
      text: text,
      secret: true,
      delivery: clips.TextDelivery.copy,
    )).token,
  );

  @override
  Future<void> restoreClipboard(String token, Duration after) => _guard(
    () => clips.restoreClipboard(token: token, afterMs: after.inMilliseconds),
  );

  @override
  Stream<void> changes() => _events.libraryChanges(const {
    LibraryKind.grants,
    LibraryKind.preferences,
    LibraryKind.setMembers,
  });

  @override
  Stream<void> approvalChanges() =>
      _events.eventsOf(const {RuntimeEventKind.aiApproval});

  static Future<T> _guard<T>(Future<T> Function() request) async {
    try {
      return await request();
    } on runtime.RuntimeError catch (error) {
      throw AiToolsException(error.code, error.message);
    }
  }

  static Object _wire(Object value) => switch (value) {
    AiToolLookback(:final wireName) => wireName,
    Iterable<AiToolContentKind>() => [for (final kind in value) kind.wireName],
    _ => value,
  };

  static AiToolsPolicy _policy(api.AiPolicyView view) => AiToolsPolicy(
    includeOtherDevices: view.includeOtherDevices,
    excludedDeviceIds: view.excludedDeviceIds,
    contentKinds: {
      for (final kind in view.contentKinds)
        switch (kind) {
          api.AiContentKind.text => AiToolContentKind.text,
          api.AiContentKind.link => AiToolContentKind.link,
          api.AiContentKind.richText => AiToolContentKind.richText,
          api.AiContentKind.image => AiToolContentKind.image,
          api.AiContentKind.fileMetadata => AiToolContentKind.fileMetadata,
        },
    },
    excludedSourceApps: view.excludedSourceApps,
    defaultLookback: _lookbackOf(view.defaultLookback),
  );

  static AiToolGrant _grant(api.AiGrantView view) => AiToolGrant(
    id: view.id,
    ownerDeviceId: view.ownerDeviceId,
    ownerDeviceName: view.ownerDeviceName,
    name: view.name,
    clientName: view.clientName,
    clientExecutable: view.clientExecutable,
    scopes: {
      for (final scope in view.scopes)
        switch (scope) {
          api.AiScope.readHistory => AiToolScope.readHistory,
          api.AiScope.readSnippets => AiToolScope.readSnippets,
          api.AiScope.writeClipboard => AiToolScope.writeClipboard,
          api.AiScope.organize => AiToolScope.organize,
        },
    },
    lookback: _lookbackOf(view.lookback),
    createdAt: DateTime.fromMillisecondsSinceEpoch(view.createdAtMs),
    lastUsedDay: switch (view.lastUsedDay) {
      final day? => DateTime.fromMillisecondsSinceEpoch(
        day * _dayMs,
        isUtc: true,
      ),
      null => null,
    },
    revoked: view.revoked,
    reissueRequired: view.reissueRequired,
  );

  static List<api.AiScope> _scopes(Set<AiToolScope> scopes) => [
    for (final scope in AiToolScope.values)
      if (scopes.contains(scope))
        switch (scope) {
          AiToolScope.readHistory => api.AiScope.readHistory,
          AiToolScope.readSnippets => api.AiScope.readSnippets,
          AiToolScope.writeClipboard => api.AiScope.writeClipboard,
          AiToolScope.organize => api.AiScope.organize,
        },
  ];

  static api.AiLookback _lookback(AiToolLookback lookback) =>
      switch (lookback) {
        AiToolLookback.day => api.AiLookback.day,
        AiToolLookback.week => api.AiLookback.week,
        AiToolLookback.month => api.AiLookback.month,
        AiToolLookback.all => api.AiLookback.all,
      };

  static AiToolLookback _lookbackOf(api.AiLookback lookback) =>
      switch (lookback) {
        api.AiLookback.day => AiToolLookback.day,
        api.AiLookback.week => AiToolLookback.week,
        api.AiLookback.month => AiToolLookback.month,
        api.AiLookback.all => AiToolLookback.all,
      };
}
