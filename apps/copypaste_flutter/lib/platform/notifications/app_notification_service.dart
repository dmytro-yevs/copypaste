import 'dart:async';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter_local_notifications/flutter_local_notifications.dart';

import '../android/android_capture_setup_gateway.dart';

/// One plugin owner: capture notifications cannot replace pairing callbacks.
class AppNotificationService {
  AppNotificationService({
    FlutterLocalNotificationsPlugin? plugin,
    AndroidCaptureSetupGateway? androidSetup,
  }) : plugin = plugin ?? FlutterLocalNotificationsPlugin(),
       _androidSetup = androidSetup;
  static final instance = AppNotificationService();
  final FlutterLocalNotificationsPlugin plugin;
  final AndroidCaptureSetupGateway? _androidSetup;
  final _responses = StreamController<String>.broadcast();
  Stream<String> get responses => _responses.stream;
  Future<void>? _initializing;

  Future<void> initialize() => _initializing ??= _initialize();

  /// The one notification-permission request for every feature. Android uses
  /// the native setup request, which also refreshes setup state observers.
  /// Windows and Linux have no runtime notification permission to request.
  Future<bool> requestPermission({bool sound = false}) async {
    if (Platform.isAndroid) {
      final setup = _androidSetup ?? MethodChannelAndroidCaptureSetupGateway();
      return (await setup.requestNotifications()).notificationGranted;
    }
    await initialize();
    if (Platform.isMacOS) {
      return await plugin
              .resolvePlatformSpecificImplementation<
                MacOSFlutterLocalNotificationsPlugin
              >()
              ?.requestPermissions(alert: true, badge: false, sound: sound) ??
          false;
    }
    return Platform.isWindows || Platform.isLinux;
  }

  Future<void> _initialize() async {
    try {
      final initialized = await plugin.initialize(
        settings: const InitializationSettings(
          android: AndroidInitializationSettings('@mipmap/ic_launcher'),
          macOS: DarwinInitializationSettings(
            requestAlertPermission: false,
            requestBadgePermission: false,
            requestSoundPermission: false,
          ),
          windows: WindowsInitializationSettings(
            appName: 'CopyPaste',
            appUserModelId: 'com.copypaste.app',
            guid: 'f112c130-c009-4a8c-b95a-16608410ce76',
          ),
          linux: LinuxInitializationSettings(defaultActionName: 'Open'),
        ),
        onDidReceiveNotificationResponse: (response) {
          final payload = response.payload;
          if (payload != null) _responses.add(payload);
        },
      );
      // Windows reports native registration failures as false instead of an
      // exception. Let callers retry, rather than caching an unusable owner.
      // Darwin can return false for notification permission without failing
      // to initialize the plugin, so retain its existing permission flow.
      if (defaultTargetPlatform == TargetPlatform.windows &&
          initialized != true) {
        throw StateError('Windows notifications could not be initialized.');
      }
    } catch (_) {
      _initializing = null;
      rethrow;
    }
  }
}
