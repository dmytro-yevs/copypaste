import 'package:flutter/services.dart';

import 'android_shizuku_state.dart';
export 'android_shizuku_state.dart';

class AndroidCaptureSetupState {
  const AndroidCaptureSetupState({
    required this.packageName,
    required this.privilegedGrants,
    required this.notificationGranted,
    required this.batteryExempt,
    required this.captureEnabled,
    required this.serviceRunning,
    required this.lastCaptureAtMs,
    this.observedAtMs = 0,
    required this.shizuku,
    required this.adbCommands,
  });

  final String packageName;
  final bool privilegedGrants;
  final bool notificationGranted;
  final bool batteryExempt;
  final bool captureEnabled;
  final bool serviceRunning;
  final int lastCaptureAtMs;
  final int observedAtMs;
  final AndroidShizukuState shizuku;
  final List<String> adbCommands;

  bool get backgroundCaptureRunning =>
      privilegedGrants &&
      notificationGranted &&
      captureEnabled &&
      serviceRunning;
}

abstract interface class AndroidCaptureSetupGateway {
  Stream<AndroidCaptureSetupState> get changes;

  Future<AndroidCaptureSetupState> state();

  Future<AndroidCaptureSetupState> requestNotifications();

  Future<bool> requestBatteryExemption();

  Future<bool> openShizuku();

  Future<AndroidCaptureSetupState> applyShizukuGrants();

  Future<AndroidCaptureSetupState> startCapture();

  Future<AndroidCaptureSetupState> stopCapture();

  Future<bool> setForegroundCaptureEnabled(bool enabled);
}

class MethodChannelAndroidCaptureSetupGateway
    implements AndroidCaptureSetupGateway {
  MethodChannelAndroidCaptureSetupGateway({
    MethodChannel? channel,
    EventChannel? events,
  }) : _channel =
           channel ?? const MethodChannel('com.copypaste.app/android_capture'),
       _events =
           events ??
           const EventChannel('com.copypaste.app/android_capture/state');

  final MethodChannel _channel;
  final EventChannel _events;

  @override
  late final Stream<AndroidCaptureSetupState> changes = _events
      .receiveBroadcastStream()
      .map((event) => _decode(Map<String, Object?>.from(event as Map)));

  @override
  Future<AndroidCaptureSetupState> state() => _state('state');

  @override
  Future<AndroidCaptureSetupState> requestNotifications() =>
      _state('requestNotifications');

  @override
  Future<bool> requestBatteryExemption() async =>
      await _channel.invokeMethod<bool>('requestBatteryExemption') ?? false;

  @override
  Future<bool> openShizuku() async =>
      await _channel.invokeMethod<bool>('openShizuku') ?? false;

  @override
  Future<AndroidCaptureSetupState> applyShizukuGrants() =>
      _state('applyShizukuGrants');

  @override
  Future<AndroidCaptureSetupState> startCapture() => _state('startCapture');

  @override
  Future<AndroidCaptureSetupState> stopCapture() => _state('stopCapture');

  @override
  Future<bool> setForegroundCaptureEnabled(bool enabled) async =>
      await _channel.invokeMethod<bool>('setForegroundCaptureEnabled', {
        'enabled': enabled,
      }) ??
      false;

  Future<AndroidCaptureSetupState> _state(String method) async {
    final raw = await _channel.invokeMapMethod<String, Object?>(method);
    if (raw == null) throw StateError('Android capture state is unavailable.');
    return _decode(raw);
  }

  static AndroidCaptureSetupState _decode(Map<String, Object?> raw) {
    final shizuku = Map<String, Object?>.from(
      raw['shizuku'] as Map<Object?, Object?>? ?? const {},
    );
    return AndroidCaptureSetupState(
      packageName: raw['packageName'] as String? ?? '',
      privilegedGrants: raw['privilegedGrants'] as bool? ?? false,
      notificationGranted: raw['notificationGranted'] as bool? ?? false,
      batteryExempt: raw['batteryExempt'] as bool? ?? false,
      captureEnabled: raw['captureEnabled'] as bool? ?? false,
      serviceRunning: raw['serviceRunning'] as bool? ?? false,
      lastCaptureAtMs: (raw['lastCaptureAtMs'] as num?)?.toInt() ?? 0,
      observedAtMs: (raw['observedAtMs'] as num?)?.toInt() ?? 0,
      shizuku: AndroidShizukuState(
        supported: shizuku['supported'] as bool? ?? false,
        installed: shizuku['installed'] as bool? ?? false,
        running: shizuku['running'] as bool? ?? false,
        permission: shizuku['permission'] as bool? ?? false,
      ),
      adbCommands: List<String>.unmodifiable(
        (raw['adbCommands'] as List<Object?>? ?? const []).whereType<String>(),
      ),
    );
  }
}
