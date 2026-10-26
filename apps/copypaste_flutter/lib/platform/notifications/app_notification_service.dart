import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter_local_notifications/flutter_local_notifications.dart';

/// One plugin owner: capture notifications cannot replace pairing callbacks.
class AppNotificationService {
  AppNotificationService({FlutterLocalNotificationsPlugin? plugin})
    : plugin = plugin ?? FlutterLocalNotificationsPlugin();
  static final instance = AppNotificationService();
  final FlutterLocalNotificationsPlugin plugin;
  final _responses = StreamController<String>.broadcast();
  Stream<String> get responses => _responses.stream;
  Future<void>? _initializing;

  Future<void> initialize() => _initializing ??= _initialize();
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
