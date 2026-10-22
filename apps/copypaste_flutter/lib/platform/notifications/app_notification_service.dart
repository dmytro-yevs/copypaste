import 'dart:async';
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
      await plugin.initialize(
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
    } catch (_) {
      _initializing = null;
      rethrow;
    }
  }
}
