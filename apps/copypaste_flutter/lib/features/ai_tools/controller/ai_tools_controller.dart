import 'dart:async';

import 'package:flutter/foundation.dart';

import '../../../l10n/l10n.dart';
import '../../history/models/history_models.dart';
import '../models/ai_tools_models.dart';
import '../repository/ai_tools_repository.dart';

enum AiToolsLoadStatus { idle, loading, ready, failed }

/// Settings → AI tools state. It loads when a settings view attaches and
/// drops everything, including its change subscription, when the last one
/// detaches. [serving] is true on desktops, which host the MCP server;
/// Android shows the synced policy and grants only.
class AiToolsController extends ChangeNotifier {
  AiToolsController({
    required AiToolsRepository repository,
    required this.serving,
  }) : _repository = repository;

  /// How long a copied credential stays on the clipboard.
  static const secretClipboardLifetime = Duration(seconds: 60);

  final bool serving;
  final AiToolsRepository _repository;

  AiToolsLoadStatus _status = AiToolsLoadStatus.idle;
  AiToolsStatus _serverStatus = const AiToolsStatus(enabled: false);
  AiToolsPolicy _policy = const AiToolsPolicy();
  List<AiToolGrant> _grants = const [];
  AiToolsExposureOptions _options = const AiToolsExposureOptions();
  String? _localDeviceId;
  String? _loadError;
  String? _actionError;
  bool _busy = false;
  int _attached = 0;
  int _generation = 0;
  StreamSubscription<void>? _changes;
  bool _disposed = false;

  AiToolsLoadStatus get status => _status;
  AiToolsStatus get serverStatus => _serverStatus;
  AiToolsPolicy get policy => _policy;
  List<AiToolGrant> get grants => _grants;
  AiToolsExposureOptions get options => _options;
  String? get loadError => _loadError;
  String? get actionError => _actionError;
  bool get busy => _busy;

  bool isLocal(AiToolGrant grant) =>
      _localDeviceId != null && grant.ownerDeviceId == _localDeviceId;

  void attach() {
    if (_attached++ > 0) return;
    _changes = _repository.changes().listen(
      (_) => unawaited(_reload()),
      onError: (Object _) {},
    );
    unawaited(_reload());
  }

  void detach() {
    if (_attached == 0 || --_attached > 0) return;
    unawaited(_changes?.cancel());
    _changes = null;
    _generation++;
    _grants = const [];
    _options = const AiToolsExposureOptions();
    _status = AiToolsLoadStatus.idle;
  }

  Future<void> retry() => _reload();

  Future<void> _reload() async {
    final generation = ++_generation;
    if (_status != AiToolsLoadStatus.ready) {
      _status = AiToolsLoadStatus.loading;
      notifyListeners();
    }
    try {
      final loaded = await Future.wait<Object?>([
        _repository.policy(),
        _repository.grants(),
        _repository.exposureOptions(),
        _repository.localDeviceId(),
        if (serving) _repository.status(),
      ]);
      if (_disposed || generation != _generation) return;
      _policy = loaded[0]! as AiToolsPolicy;
      _grants = loaded[1]! as List<AiToolGrant>;
      _options = loaded[2]! as AiToolsExposureOptions;
      _localDeviceId = loaded[3] as String?;
      if (serving) _serverStatus = loaded[4]! as AiToolsStatus;
      _status = AiToolsLoadStatus.ready;
      _loadError = null;
    } on Object catch (error) {
      if (_disposed || generation != _generation) return;
      _status = AiToolsLoadStatus.failed;
      _loadError = _message(error, AppStrings.current.aiToolsLoadFailed);
    }
    notifyListeners();
  }

  Future<bool> _mutate(Future<void> Function() change) async {
    if (_busy) return false;
    _busy = true;
    _actionError = null;
    notifyListeners();
    try {
      await change();
      return true;
    } on Object catch (error) {
      _actionError = _message(error, AppStrings.current.aiToolsChangeFailed);
      return false;
    } finally {
      _busy = false;
      notifyListeners();
    }
  }

  static String _message(Object error, String fallback) =>
      error is AiToolsException ? error.message : fallback;

  Future<bool> setEnabled(bool enabled) => _mutate(() async {
    final applied = await _repository.setEnabled(enabled);
    _serverStatus = AiToolsStatus(
      enabled: applied,
      contributingDevices: _serverStatus.contributingDevices,
    );
  });

  Future<bool> _setPolicy(String key, Object value) =>
      _mutate(() async => _policy = await _repository.setPolicy(key, value));

  Future<bool> _setMember(String set, String member, bool present) => _mutate(
    () async => _policy = await _repository.setPolicyMember(
      set,
      member,
      present: present,
    ),
  );

  Future<bool> setIncludeOtherDevices(bool include) =>
      _setPolicy(AiToolsPolicyKey.includeOtherDevices, include);

  Future<bool> setDeviceIncluded(String deviceId, bool included) =>
      _setMember(AiToolsPolicySet.excludedDevices, deviceId, !included);

  Future<bool> setContentKind(AiToolContentKind kind, bool included) {
    final kinds = {..._policy.contentKinds};
    included ? kinds.add(kind) : kinds.remove(kind);
    return _setPolicy(AiToolsPolicyKey.contentKinds, [
      for (final value in AiToolContentKind.values)
        if (kinds.contains(value)) value,
    ]);
  }

  Future<bool> setAppExcluded(String appId, bool excluded) =>
      _setMember(AiToolsPolicySet.excludedApps, appId, excluded);

  Future<bool> setDefaultLookback(AiToolLookback lookback) =>
      _setPolicy(AiToolsPolicyKey.defaultLookback, lookback);

  /// Creates a connection; the answer holds the only copy of its credential.
  Future<AiToolGrantCreated?> createGrant({
    required String name,
    required AiToolClient client,
    required Set<AiToolScope> scopes,
    required AiToolLookback lookback,
  }) async {
    AiToolGrantCreated? created;
    final ok = await _mutate(() async {
      created = await _repository.createGrant(
        name: name,
        client: client,
        scopes: scopes,
        lookback: lookback,
      );
      _grants = [created!.grant, ..._grants];
    });
    return ok ? created : null;
  }

  Future<bool> updateGrant(
    AiToolGrant grant, {
    required String name,
    required Set<AiToolScope> scopes,
    required AiToolLookback lookback,
  }) => _mutate(() async {
    final updated = await _repository.updateGrant(
      grant.id,
      name: name == grant.name ? null : name,
      scopes: setEquals(scopes, grant.scopes) ? null : scopes,
      lookback: lookback == grant.lookback ? null : lookback,
    );
    _replace(updated);
  });

  Future<bool> revokeGrant(AiToolGrant grant) =>
      _mutate(() async => _replace(await _repository.revokeGrant(grant.id)));

  Future<bool> deleteGrant(AiToolGrant grant) => _mutate(() async {
    await _repository.deleteGrant(grant.id);
    _grants = [..._grants.where((entry) => entry.id != grant.id)];
  });

  void _replace(AiToolGrant grant) => _grants = [
    for (final entry in _grants) entry.id == grant.id ? grant : entry,
  ];

  Future<List<AiAccessEntry>> accessLog(AiToolGrant grant) =>
      _repository.accessLog(grant.id);

  Future<HistorySourceAppIcon?> sourceAppIcon(String iconId) =>
      _repository.sourceAppIcon(iconId);

  /// Copies a configuration that embeds a credential without the system
  /// clipboard writer: the write is concealed and transient, so it is not
  /// captured into history, and the previous clipboard returns after
  /// [secretClipboardLifetime] unless something else was copied meanwhile.
  Future<bool> copySecret(String text) async {
    try {
      final token = await _repository.copySecret(text);
      if (token != null) {
        await _repository.restoreClipboard(token, secretClipboardLifetime);
      }
      return true;
    } on Object {
      return false;
    }
  }

  @override
  void notifyListeners() {
    if (!_disposed) super.notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    unawaited(_changes?.cancel());
    super.dispose();
  }
}
