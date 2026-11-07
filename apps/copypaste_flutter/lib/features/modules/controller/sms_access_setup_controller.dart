import 'dart:async';

import 'package:flutter/foundation.dart';

import '../../../platform/clipboard/clipboard_writer.dart';
import '../../../l10n/l10n.dart';
import '../models/module_models.dart';
import '../repository/module_access_repository.dart';

/// Owns permission setup only while its drawer is visible and resumed.
class SmsAccessSetupController extends ChangeNotifier {
  SmsAccessSetupController({
    required ModuleAccessRepository access,
    ClipboardWriter clipboard = const SystemClipboardWriter(),
  }) : _access = access,
       _clipboard = clipboard;

  final ModuleAccessRepository _access;
  final ClipboardWriter _clipboard;
  SmsModuleAccessState? _state;
  SmsModuleAccessState? get state => _state;
  int _methodIndex = 0;
  int get methodIndex => _methodIndex;
  bool _busy = false;
  bool get busy => _busy;
  bool _loading = true;
  bool get loading => _loading;
  String? _errorMessage;
  String? get errorMessage => _errorMessage;
  bool _disposed = false;
  bool _reading = false;
  bool _refreshFailed = false;
  bool _automaticGrantsAttempted = false;
  int _revision = 0;
  StreamSubscription<SmsModuleAccessState>? _monitor;

  void selectMethod(int index) {
    if (_busy || _disposed || index == _methodIndex) return;
    _methodIndex = index;
    _errorMessage = null;
    notifyListeners();
    _applyAuthorizedSetup();
  }

  void setMonitoring(bool enabled) {
    if (_disposed) return;
    if (!enabled) {
      unawaited(_monitor?.cancel());
      _monitor = null;
      return;
    }
    if (_monitor != null) return;
    // The native stream owns change detection, including ADB grants.
    _monitor = _access.smsChanges.listen(
      (state) {
        if (_disposed) return;
        _acceptState(state);
        _loading = false;
        notifyListeners();
        _applyAuthorizedSetup();
      },
      onError: (Object _) {
        if (_disposed) return;
        _refreshFailed = true;
        _errorMessage = AppStrings.current.modulesSmsVerifyFailed;
        notifyListeners();
      },
    );
    unawaited(refresh());
  }

  Future<void> refresh() async {
    if (_disposed || _busy || _reading) return;
    _reading = true;
    final revision = _revision;
    try {
      final state = await _access.smsState();
      if (!_disposed && revision == _revision) _acceptState(state);
    } catch (_) {
      if (!_disposed && revision == _revision) {
        _refreshFailed = true;
        _errorMessage = AppStrings.current.modulesSmsVerifyFailed;
      }
    } finally {
      _reading = false;
      if (!_disposed) {
        _loading = false;
        notifyListeners();
        _applyAuthorizedSetup();
      }
    }
  }

  Future<void> requestNotifications() => _run(() async {
    _acceptState(await _access.requestSmsNotifications());
    if (_state?.notificationGranted != true) {
      throw ModulesException(AppStrings.current.modulesSmsAllowNotifications);
    }
  });

  Future<void> openShizuku() => _run(() async {
    if (!await _access.openShizuku()) {
      throw ModulesException(AppStrings.current.sharedShizukuOpenFailed);
    }
  });

  Future<void> applyAccess() => _run(() async {
    _automaticGrantsAttempted = true;
    _acceptState(await _access.configureSms());
    if (_state?.granted != true) {
      throw ModulesException(AppStrings.current.modulesSmsNotGranted);
    }
  });

  Future<bool> copyAdbCommands() async {
    final commands = _state?.adbCommands ?? '';
    if (commands.isEmpty || _disposed) return false;
    try {
      await _clipboard.writeText(commands);
      return true;
    } catch (_) {
      if (!_disposed) {
        _errorMessage = AppStrings.current.sharedSetupCommandsCopyFailed;
        notifyListeners();
      }
      return false;
    }
  }

  void _acceptState(SmsModuleAccessState state) {
    if (_disposed) return;
    _state = state;
    if (state.granted || _refreshFailed) _errorMessage = null;
    _refreshFailed = false;
    if (!state.shizuku.running || !state.shizuku.permission) {
      _automaticGrantsAttempted = false;
    }
  }

  void _applyAuthorizedSetup() {
    final state = _state;
    if (_disposed ||
        _monitor == null ||
        _busy ||
        _methodIndex != 0 ||
        _automaticGrantsAttempted ||
        state == null ||
        state.smsGranted ||
        !state.notificationGranted ||
        !state.shizuku.supported ||
        !state.shizuku.installed ||
        !state.shizuku.running ||
        !state.shizuku.permission) {
      return;
    }
    unawaited(applyAccess());
  }

  Future<void> _run(Future<void> Function() action) async {
    if (_busy || _disposed) return;
    _busy = true;
    _revision++;
    _errorMessage = null;
    notifyListeners();
    try {
      await action();
    } catch (error) {
      if (!_disposed) {
        _errorMessage = error is ModulesException
            ? error.message
            : AppStrings.current.modulesSmsConfigureFailed;
      }
    } finally {
      if (!_disposed) {
        _busy = false;
        notifyListeners();
      }
    }
  }

  @override
  void dispose() {
    _disposed = true;
    unawaited(_monitor?.cancel());
    super.dispose();
  }
}
