import 'dart:async';

import 'package:flutter/foundation.dart';

import '../../../platform/clipboard/clipboard_writer.dart';
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
  Timer? _monitor;

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
      _monitor?.cancel();
      _monitor = null;
      return;
    }
    if (_monitor != null) return;
    // ADB can change grants without a Shizuku callback or an app resume.
    _monitor = Timer.periodic(const Duration(seconds: 1), (_) {
      unawaited(refresh());
    });
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
        _errorMessage = 'SMS access could not be verified.';
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
      throw const ModulesException(
        'Allow notifications to keep SMS Codes active in the background.',
      );
    }
  });

  Future<void> openShizuku() => _run(() async {
    if (!await _access.openShizuku()) {
      throw const ModulesException('Shizuku could not be opened.');
    }
  });

  Future<void> applyAccess() => _run(() async {
    _automaticGrantsAttempted = true;
    _acceptState(await _access.configureSms());
    if (_state?.granted != true) {
      throw const ModulesException('SMS access was not granted.');
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
        _errorMessage = 'Setup commands could not be copied.';
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
            : 'SMS access could not be configured.';
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
    _monitor?.cancel();
    super.dispose();
  }
}
