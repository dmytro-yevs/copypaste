import 'dart:async';

import 'package:copypaste_flutter/app/navigation/navigation.dart';
import 'package:copypaste_flutter/app/shell/app_shell.dart';
import 'package:copypaste_flutter/features/devices/devices.dart';
import 'package:copypaste_flutter/features/notifications/notifications_controller.dart';
import 'package:copypaste_flutter/features/notifications/notifications_view.dart';
import 'package:copypaste_flutter/platform/notifications/pairing_notification_port.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';

void main() {
  late _Devices devices;
  late _Platform platform;
  late ValueNotifier<bool> visible;
  late NotificationsController inbox;
  late int opened;
  setUp(() {
    devices = _Devices();
    platform = _Platform();
    visible = ValueNotifier(false);
    opened = 0;
    inbox = NotificationsController(
      devices: devices,
      platform: platform,
      visible: visible,
      openPairing: () => opened++,
    );
  });
  tearDown(() {
    inbox.dispose();
    devices.dispose();
    visible.dispose();
  });
  test(
    'one background notification opens the current request and rejects stale actions',
    () async {
      devices.emit('request', PairingState.awaitingConfirmation);
      devices.emit('request', PairingState.awaitingConfirmation);
      await Future<void>.delayed(Duration.zero);
      expect(platform.shown, ['request']);
      expect(inbox.unread, 1);
      platform.responses.add('request');
      await Future<void>.delayed(Duration.zero);
      expect(opened, 1);
      devices.emit('request', PairingState.timedOut);
      platform.responses.add('request');
      await Future<void>.delayed(Duration.zero);
      expect(opened, 1);
      expect(inbox.items.single.title, 'Connection request expired');
      expect(platform.dismissed, 1);
    },
  );
  test(
    'foreground requests open pairing; closing updates the bounded inbox',
    () {
      visible.value = true;
      devices.emit('first', PairingState.awaitingConfirmation);
      expect(opened, 1);
      expect(platform.shown, isEmpty);
      devices.clear();
      expect(inbox.items.single.state, PairingState.cancelled);
      for (var i = 0; i < 55; i++) {
        devices.emit('request-$i', PairingState.awaitingConfirmation);
        devices.emit('request-$i', PairingState.confirmed);
      }
      expect(inbox.items.length, NotificationsController.maxEntries);
      inbox.markAllRead();
      inbox.clearRead();
      expect(inbox.items, isEmpty);
    },
  );
  testWidgets(
    'Notifications stays beside Downloads in the compact shared header',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(320, 640));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final navigation = AppNavigationController();
      addTearDown(navigation.dispose);
      await tester.pumpWidget(
        ShadcnApp(
          theme: AppTheme.light,
          home: AppShell(
            controller: navigation,
            destinations: {
              for (final page in AppDestination.values)
                page: const SizedBox.expand(),
            },
            headerActions: {
              AppDestination.history: [
                NotificationsHeaderAction(controller: inbox),
                for (final icon in [
                  LucideIcons.download,
                  LucideIcons.refreshCw,
                  LucideIcons.clipboard,
                ])
                  Button.secondary(
                    style: AppTheme.controlButtonStyle(
                      const ButtonStyle.secondaryIcon(),
                    ),
                    onPressed: () {},
                    child: Icon(icon),
                  ),
              ],
            },
          ),
        ),
      );
      await tester.pumpAndSettle();
      final bell = find.byKey(const ValueKey('notifications-open'));
      final download = find.byIcon(LucideIcons.download);
      final title = find.descendant(
        of: find.byType(AppBar),
        matching: find.text('History'),
      );
      expect(find.byType(AppBar), findsOneWidget);
      expect(tester.getCenter(title).dy, closeTo(tester.getCenter(bell).dy, 1));
      expect(
        tester.getCenter(bell).dy,
        closeTo(tester.getCenter(download).dy, 1),
      );
      expect(
        tester.getCenter(bell).dx,
        lessThan(tester.getCenter(download).dx),
      );
      await tester.tap(bell);
      await tester.pumpAndSettle();
      expect(find.text('No notifications'), findsOneWidget);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    },
  );

  testWidgets('bell shows requests and a review action in the shared drawer', (
    tester,
  ) async {
    visible.value = false;
    devices.emit('request', PairingState.awaitingConfirmation);
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        home: DrawerOverlay(
          child: Scaffold(
            headers: [
              AppBar(trailing: [NotificationsHeaderAction(controller: inbox)]),
            ],
            child: const SizedBox.expand(),
          ),
        ),
      ),
    );
    await tester.tap(find.byKey(const ValueKey('notifications-open')));
    await tester.pumpAndSettle();
    expect(find.text('Connection request'), findsOneWidget);
    expect(find.text('Nearby laptop'), findsOneWidget);
    expect(find.text('Review'), findsOneWidget);
    expect(find.text('Clear read notifications'), findsOneWidget);
    await tester.tap(find.text('Review'));
    await tester.pumpAndSettle();
    expect(opened, 1);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
  });
}

class _Devices extends DevicesController {
  _Devices() : super(gateway: _Gateway(), captureProtection: _Protection());
  PairingCeremony? value;
  @override
  PairingCeremony? get pairing => value;
  @override
  bool get incomingPairing => true;
  void emit(String id, PairingState state) {
    value = PairingCeremony(id: id, state: state, peerName: 'Nearby laptop');
    notifyListeners();
  }

  void clear() {
    value = null;
    notifyListeners();
  }
}

class _Gateway implements DevicesGateway {
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Protection implements PairingCaptureProtection {
  @override
  Future<bool> setEnabled(bool enabled) async => true;
}

class _Platform implements PairingNotificationPort {
  @override
  Future<bool> requestPermission() async => true;
  final responses = StreamController<String>.broadcast();
  final shown = <String>[];
  int dismissed = 0;
  @override
  Stream<String> get opened => responses.stream;
  @override
  Future<void> showRequest({
    required String requestId,
    required String peerName,
  }) async {
    shown.add(requestId);
  }

  @override
  Future<void> dismiss() async {
    dismissed++;
  }
}
