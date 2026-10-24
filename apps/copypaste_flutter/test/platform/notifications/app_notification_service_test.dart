import 'dart:async';

import 'package:copypaste_flutter/platform/notifications/app_notification_service.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_local_notifications/flutter_local_notifications.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  tearDown(() => debugDefaultTargetPlatformOverride = null);

  for (final result in <bool?>[false, null]) {
    test('Windows initialization result $result permits a retry', () async {
      debugDefaultTargetPlatformOverride = TargetPlatform.windows;
      final plugin = _Notifications()..result = result;
      final service = AppNotificationService(plugin: plugin);

      await expectLater(service.initialize(), throwsStateError);
      plugin.result = true;
      await service.initialize();
      await service.initialize();

      expect(plugin.initializations, 2);
    });
  }

  test('concurrent initialization shares one callback owner', () async {
    debugDefaultTargetPlatformOverride = TargetPlatform.windows;
    final plugin = _Notifications()..pending = Completer<bool?>();
    final service = AppNotificationService(plugin: plugin);
    final first = service.initialize();
    final second = service.initialize();

    expect(identical(first, second), isTrue);
    expect(plugin.initializations, 1);
    plugin.pending!.complete(true);
    await Future.wait([first, second]);

    final response = service.responses.first;
    plugin.onResponse!(
      const NotificationResponse(
        notificationResponseType: NotificationResponseType.selectedNotification,
        payload: 'pairing:request',
      ),
    );
    expect(await response, 'pairing:request');
  });

  test(
    'macOS permission denial keeps the initialized callback owner',
    () async {
      debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
      final plugin = _Notifications()..result = false;
      final service = AppNotificationService(plugin: plugin);

      await service.initialize();
      await service.initialize();

      expect(plugin.initializations, 1);
    },
  );
}

class _Notifications implements FlutterLocalNotificationsPlugin {
  bool? result = true;
  Completer<bool?>? pending;
  int initializations = 0;
  DidReceiveNotificationResponseCallback? onResponse;

  @override
  Future<bool?> initialize({
    required InitializationSettings settings,
    DidReceiveNotificationResponseCallback? onDidReceiveNotificationResponse,
    DidReceiveBackgroundNotificationResponseCallback?
    onDidReceiveBackgroundNotificationResponse,
  }) async {
    initializations += 1;
    onResponse = onDidReceiveNotificationResponse;
    return pending == null ? result : pending!.future;
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}
