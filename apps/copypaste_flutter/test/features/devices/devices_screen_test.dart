import 'dart:async';
import 'dart:convert';

import 'package:copypaste_flutter/features/devices/devices.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  testWidgets('switches from the right inspector below 800 logical pixels', (
    tester,
  ) async {
    final gateway = _ScreenGateway(peers: const [], discovered: const []);
    final controller = DevicesController(
      gateway: gateway,
      captureProtection: _CaptureProtection(),
    );
    addTearDown(controller.dispose);

    await _pumpDevices(tester, controller, size: const Size(800, 900));
    await tester.tap(find.byKey(const ValueKey<String>('enter-pairing-code')));
    await tester.pumpAndSettle();

    expect(
      find.byKey(const ValueKey<String>('devices-pairing-inspector')),
      findsOneWidget,
    );
    expect(
      find.byKey(const ValueKey<String>('devices-pairing-drawer')),
      findsNothing,
    );

    await tester.tap(
      find.byKey(const ValueKey<String>('close-pairing-inspector')),
    );
    await tester.pumpAndSettle();
    await tester.binding.setSurfaceSize(const Size(799, 900));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey<String>('enter-pairing-code')));
    await tester.pumpAndSettle();

    expect(
      find.byKey(const ValueKey<String>('devices-pairing-inspector')),
      findsNothing,
    );
    expect(
      find.byKey(const ValueKey<String>('devices-pairing-drawer')),
      findsOneWidget,
    );
    expect(tester.takeException(), isNull);
  });

  testWidgets('uses compact surfaced cards for current and trusted devices', (
    tester,
  ) async {
    final gateway = _ScreenGateway();
    final controller = DevicesController(
      gateway: gateway,
      captureProtection: _CaptureProtection(),
    );
    addTearDown(controller.dispose);

    await _pumpDevices(tester, controller);

    for (final key in [
      'rescan-devices',
      'pair-device',
      'scan-pairing-qr',
      'enter-pairing-code',
    ]) {
      expect(
        find.ancestor(
          of: find.byKey(ValueKey<String>(key)),
          matching: find.byType(AppBar),
        ),
        findsOneWidget,
      );
    }
    expect(
      find.descendant(
        of: find.byKey(const ValueKey<String>('pair-device')),
        matching: find.byIcon(LucideIcons.qrCode),
      ),
      findsOneWidget,
    );
    expect(
      find.byKey(const ValueKey<String>('this-device-card')),
      findsOneWidget,
    );
    expect(
      find.byKey(const ValueKey<String>('peer-device-card-peer')),
      findsOneWidget,
    );
    for (final name in ['Desktop', 'Phone', 'Tablet']) {
      expect(find.text(name), findsOneWidget);
    }
    expect(find.text('Desktop · macOS 15.6'), findsOneWidget);
    expect(find.text('Phone · Android 16'), findsOneWidget);
    expect(find.text('This device'), findsNothing);
    expect(find.text('Trusted'), findsNothing);
    expect(find.text('Available'), findsNothing);
    expect(find.text('24 ms'), findsNothing);
    expect(find.text('Already paired'), findsOneWidget);
    expect(find.text('Your devices'), findsNothing);
    expect(find.text('Trusted devices'), findsNothing);

    final thisDeviceFinder = find.byKey(
      const ValueKey<String>('this-device-card'),
    );
    final thisDevice = tester.widget<Button>(thisDeviceFinder);
    final thisDeviceContext = tester.element(thisDeviceFinder);
    final decoration = thisDevice.style.decoration(thisDeviceContext, const {});
    expect(decoration, isA<BoxDecoration>());
    expect(
      (decoration as BoxDecoration).color,
      Theme.of(thisDeviceContext).colorScheme.card,
    );
    expect(decoration.border, isNotNull);

    final iconTile = find.descendant(
      of: thisDeviceFinder,
      matching: find.byType(Card),
    );
    expect(iconTile, findsOneWidget);
    expect(tester.getSize(iconTile), const Size.square(48));

    await tester.tap(find.byKey(const ValueKey<String>('pair-device')));
    await tester.pump();

    expect(find.text('Ready to pair'), findsOneWidget);
    expect(
      find.byKey(const ValueKey<String>('pairing-invite-qr')),
      findsOneWidget,
    );
    expect(gateway.session.revealInviteCalls, 1);
  });

  testWidgets('opens trusted device details in the shared inspector', (
    tester,
  ) async {
    final controller = DevicesController(
      gateway: _ScreenGateway(),
      captureProtection: _CaptureProtection(),
    );
    addTearDown(controller.dispose);

    await _pumpDevices(tester, controller);
    await tester.tap(
      find.byKey(const ValueKey<String>('peer-device-card-peer')),
    );
    await tester.pumpAndSettle();

    expect(
      find.byKey(const ValueKey<String>('devices-details-inspector')),
      findsOneWidget,
    );
    expect(find.text('Android 16'), findsOneWidget);
    expect(find.text('192.0.2.20:47654'), findsOneWidget);
    expect(find.text('Sync now'), findsOneWidget);
    expect(find.text('Unpair'), findsOneWidget);
    expect(find.text('Revoke pairing'), findsOneWidget);
    expect(find.text('Trusted'), findsOneWidget);
    final metadata = find.byKey(
      const ValueKey<String>('device-details-metadata'),
    );
    expect(metadata, findsOneWidget);
    final table = tester.widget<Table>(metadata);
    final tableContext = tester.element(metadata);
    expect(table.theme, isNull);
    for (final row in table.rows!) {
      final border = row
          .buildDefaultTheme(tableContext)
          .border!
          .resolve(const <WidgetState>{})!;
      expect(border.top.style, BorderStyle.none);
      expect(border.left.style, BorderStyle.none);
      expect(border.right.style, BorderStyle.none);
      expect(border.bottom.width, 1);
    }
    for (final label in ['Status', 'Ping', 'Profile trust']) {
      expect(
        find.descendant(of: metadata, matching: find.text(label)),
        findsOneWidget,
      );
    }
    final selectedCardFinder = find.byKey(
      const ValueKey<String>('peer-device-card-peer'),
    );
    final selectedCard = tester.widget<Button>(selectedCardFinder);
    final selectedCardContext = tester.element(selectedCardFinder);
    final cardDecoration =
        selectedCard.style.decoration(selectedCardContext, const {})
            as BoxDecoration;
    expect(cardDecoration.border, isNotNull);
    expect(
      cardDecoration.color,
      Theme.of(selectedCardContext).colorScheme.border,
    );
  });

  testWidgets('uses two device columns only when each card has enough width', (
    tester,
  ) async {
    final controller = DevicesController(
      gateway: _ScreenGateway(),
      captureProtection: _CaptureProtection(),
    );
    addTearDown(controller.dispose);

    await _pumpDevices(tester, controller, size: const Size(800, 900));
    final thisDevice = find.byKey(const ValueKey<String>('this-device-card'));
    final peer = find.byKey(const ValueKey<String>('peer-device-card-peer'));
    expect(tester.getTopLeft(thisDevice).dy, tester.getTopLeft(peer).dy);

    await tester.binding.setSurfaceSize(const Size(727, 900));
    await tester.pumpAndSettle();
    expect(
      tester.getTopLeft(peer).dy,
      greaterThan(tester.getTopLeft(thisDevice).dy),
    );
  });

  testWidgets('opens device details in a bottom drawer on compact layouts', (
    tester,
  ) async {
    final controller = DevicesController(
      gateway: _ScreenGateway(),
      captureProtection: _CaptureProtection(),
    );
    addTearDown(controller.dispose);

    await _pumpDevices(tester, controller, size: const Size(390, 844));
    await tester.tap(find.byKey(const ValueKey<String>('this-device-card')));
    await tester.pumpAndSettle();

    expect(
      find.byKey(const ValueKey<String>('devices-details-drawer')),
      findsOneWidget,
    );
    expect(find.text('This device'), findsOneWidget);
    expect(find.text('Local'), findsNothing);
    final metadata = find.byKey(
      const ValueKey<String>('device-details-metadata'),
    );
    expect(metadata, findsOneWidget);
    for (final label in ['Status', 'Ping', 'Profile trust']) {
      expect(
        find.descendant(of: metadata, matching: find.text(label)),
        findsNothing,
      );
    }
    for (final label in [
      'Device type',
      'Operating system',
      'CopyPaste version',
      'Protocol version',
      'Device ID',
      'Profile provenance',
      'Profile updated',
    ]) {
      expect(
        find.descendant(of: metadata, matching: find.text(label)),
        findsOneWidget,
      );
    }

    final renameButton = find.byKey(const ValueKey<String>('rename-device'));
    await tester.ensureVisible(renameButton);
    await tester.pumpAndSettle();
    final buttonRect = tester.getRect(renameButton);
    final cardRect = tester.getRect(
      find.byKey(const ValueKey<String>('devices-details-drawer-card')),
    );
    expect(buttonRect.width, lessThan(cardRect.width));

    final renameIcon = find.descendant(
      of: renameButton,
      matching: find.byIcon(LucideIcons.pencil),
    );
    final renameLabel = find.descendant(
      of: renameButton,
      matching: find.text('Rename device'),
    );
    expect(renameIcon, findsOneWidget);
    expect(renameLabel, findsOneWidget);
    expect(
      tester.getRect(renameIcon).center.dy,
      closeTo(buttonRect.center.dy, 0.5),
    );
    expect(
      tester.getRect(renameLabel).center.dy,
      closeTo(buttonRect.center.dy, 0.5),
    );
  });

  testWidgets('escape closes pre-session pairing and wide device details', (
    tester,
  ) async {
    final controller = DevicesController(
      gateway: _ScreenGateway(),
      captureProtection: _CaptureProtection(),
    );
    addTearDown(controller.dispose);

    await _pumpDevices(tester, controller);
    await tester.tap(find.byKey(const ValueKey<String>('enter-pairing-code')));
    await tester.pumpAndSettle();
    expect(
      find.byKey(const ValueKey<String>('devices-pairing-inspector')),
      findsOneWidget,
    );

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(
      find.byKey(const ValueKey<String>('devices-pairing-inspector')),
      findsNothing,
    );

    await tester.tap(
      find.byKey(const ValueKey<String>('peer-device-card-peer')),
    );
    await tester.pumpAndSettle();
    expect(
      find.byKey(const ValueKey<String>('devices-details-inspector')),
      findsOneWidget,
    );

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(
      find.byKey(const ValueKey<String>('devices-details-inspector')),
      findsNothing,
    );
  });

  testWidgets('escape closes the compact device details drawer', (
    tester,
  ) async {
    final controller = DevicesController(
      gateway: _ScreenGateway(),
      captureProtection: _CaptureProtection(),
    );
    addTearDown(controller.dispose);

    await _pumpDevices(tester, controller, size: const Size(390, 844));
    await tester.tap(find.byKey(const ValueKey<String>('this-device-card')));
    await tester.pumpAndSettle();
    expect(
      find.byKey(const ValueKey<String>('devices-details-drawer')),
      findsOneWidget,
    );

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(
      find.byKey(const ValueKey<String>('devices-details-drawer')),
      findsNothing,
    );
  });

  testWidgets(
    'shows a device action failure inside the compact details drawer',
    (tester) async {
      final gateway = _ScreenGateway()
        ..syncError = StateError('sync unavailable');
      final controller = DevicesController(
        gateway: gateway,
        captureProtection: _CaptureProtection(),
      );
      addTearDown(controller.dispose);

      await _pumpDevices(tester, controller, size: const Size(390, 844));
      await tester.tap(
        find.byKey(const ValueKey<String>('peer-device-card-peer')),
      );
      await tester.pumpAndSettle();
      await tester.ensureVisible(find.text('Sync now'));
      await tester.tap(find.text('Sync now'));
      await tester.pumpAndSettle();

      expect(
        find.byKey(const ValueKey<String>('device-details-action-error')),
        findsOneWidget,
      );
      expect(find.textContaining('sync unavailable'), findsWidgets);
    },
  );

  testWidgets('omits obsolete unavailable and trusted empty states', (
    tester,
  ) async {
    final gateway = _ScreenGateway(peers: const [], discovered: const []);
    final controller = DevicesController(
      gateway: gateway,
      captureProtection: _CaptureProtection(),
    );
    addTearDown(controller.dispose);

    await _pumpDevices(tester, controller);

    expect(find.text('Your devices'), findsNothing);
    expect(
      find.text('Manage trusted devices and local discovery.'),
      findsNothing,
    );
    expect(
      find.text('Protected pairing is not ready on this device'),
      findsNothing,
    );
    expect(
      find.text('Protected pairing is not ready on this device.'),
      findsNothing,
    );
    expect(find.text('Trusted devices'), findsNothing);
    expect(find.text('No trusted devices yet'), findsNothing);
    expect(find.text('Nearby devices'), findsOneWidget);
    expect(find.text('No nearby devices found'), findsOneWidget);
    expect(
      find.byKey(const ValueKey<String>('this-device-card')),
      findsOneWidget,
    );
  });

  testWidgets('keeps header commands usable at narrow widths', (tester) async {
    final gateway = _ScreenGateway(peers: const [], discovered: const []);
    final controller = DevicesController(
      gateway: gateway,
      captureProtection: _CaptureProtection(),
    );
    addTearDown(controller.dispose);

    await _pumpDevices(tester, controller, size: const Size(320, 640));

    for (final key in [
      'rescan-devices',
      'pair-device',
      'scan-pairing-qr',
      'enter-pairing-code',
    ]) {
      expect(find.byKey(ValueKey<String>(key)), findsOneWidget);
    }
    expect(find.text('Pair device'), findsNothing);
    expect(
      find.descendant(
        of: find.byKey(const ValueKey<String>('pair-device')),
        matching: find.byIcon(LucideIcons.qrCode),
      ),
      findsOneWidget,
    );
    await tester.tap(find.byKey(const ValueKey<String>('enter-pairing-code')));
    await tester.pumpAndSettle();

    expect(
      find.byKey(const ValueKey<String>('devices-pairing-drawer')),
      findsOneWidget,
    );
    expect(
      find.byKey(const ValueKey<String>('pairing-code-input')),
      findsOneWidget,
    );
    final otpContext = tester.element(
      find.byKey(const ValueKey<String>('pairing-code-input')),
    );
    expect(Theme.of(otpContext).colorScheme.border, Colors.transparent);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
    'opens a resizable pairing inspector from every pairing command',
    (tester) async {
      final gateway = _ScreenGateway(peers: const [], discovered: const []);
      final controller = DevicesController(
        gateway: gateway,
        captureProtection: _CaptureProtection(),
      );
      addTearDown(controller.dispose);

      await _pumpDevices(tester, controller);

      await tester.tap(
        find.byKey(const ValueKey<String>('enter-pairing-code')),
      );
      await tester.pump();

      expect(
        tester
            .widget<Button>(
              find.byKey(const ValueKey<String>('this-device-card')),
            )
            .enabled,
        isFalse,
      );

      expect(
        find.byKey(const ValueKey<String>('devices-pairing-inspector')),
        findsOneWidget,
      );
      expect(
        find.byKey(const ValueKey<String>('pairing-code-input')),
        findsOneWidget,
      );
      expect(
        find.byKey(const ValueKey<String>('pairing-address-input')),
        findsOneWidget,
      );

      await tester.tap(
        find.byKey(const ValueKey<String>('close-pairing-inspector')),
      );
      await tester.pump();
      await tester.tap(find.byKey(const ValueKey<String>('scan-pairing-qr')));
      await tester.pump();

      expect(
        find.byKey(const ValueKey<String>('start-pairing-scanner')),
        findsOneWidget,
      );
      expect(
        find.byKey(const ValueKey<String>('pairing-address-input')),
        findsNothing,
      );
    },
  );

  testWidgets('hidden devices do not intercept escape', (tester) async {
    var active = true;
    final controller = DevicesController(
      gateway: _ScreenGateway(),
      captureProtection: _CaptureProtection(),
    );
    addTearDown(controller.dispose);

    await _pumpDevices(tester, controller, isActive: () => active);
    await tester.tap(find.byKey(const ValueKey<String>('this-device-card')));
    await tester.pumpAndSettle();
    expect(controller.deviceDetailsOpen, isTrue);

    active = false;
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(controller.deviceDetailsOpen, isTrue);

    active = true;
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(controller.deviceDetailsOpen, isFalse);
  });
}

Future<void> _pumpDevices(
  WidgetTester tester,
  DevicesController controller, {
  Size size = const Size(1400, 900),
  bool Function()? isActive,
}) async {
  await tester.binding.setSurfaceSize(size);
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(
    ShadcnApp(
      home: Scaffold(
        headers: [
          AppBar(
            title: const Text('Devices'),
            trailing: [DevicesHeaderActions(controller: controller)],
          ),
          const Divider(),
        ],
        child: DevicesScreen(controller: controller, isActive: isActive),
      ),
    ),
  );
  await tester.pump();
}

class _ScreenGateway implements DevicesGateway {
  _ScreenGateway({List<DevicePeer>? peers, List<DiscoveredDevice>? discovered})
    : peers =
          peers ??
          [
            DevicePeer(
              id: 'peer',
              name: 'Phone',
              lastSeen: DateTime.utc(2026, 10, 3),
              online: true,
              details: _details(
                name: 'Phone',
                platform: DevicePlatform.android,
                deviceClass: DeviceClass.phone,
                osName: 'Android',
                osVersion: '16',
                presence: DevicePresence.online,
                latency: const Duration(milliseconds: 24),
                endpoint: '192.0.2.20:47654',
              ),
            ),
          ],
      discovered =
          discovered ??
          [
            DiscoveredDevice(
              id: 'paired',
              name: 'Tablet',
              address: '192.0.2.20:47654',
              paired: true,
              lastSeen: DateTime.utc(2026, 10, 3),
            ),
          ];

  final _ScreenSession session = _ScreenSession();
  final List<DevicePeer> peers;
  final List<DiscoveredDevice> discovered;
  Object? syncError;

  @override
  Stream<void> get changes => const Stream<void>.empty();

  @override
  Future<DevicesPairingSession> createInvitation() async => session;

  @override
  Future<DevicesPairingSession> joinFromProtectedInput({
    required String code,
    required String address,
  }) async => session;

  @override
  Future<DevicesPairingSession> joinPairingUri(String uri) async => session;

  @override
  Future<DevicesSnapshot> load() async => DevicesSnapshot(
    thisDevice: ThisDevice(
      id: 'this-device-id',
      name: 'Desktop',
      appVersion: '1.0.0',
      protocolVersion: 1,
      details: _details(
        name: 'Desktop',
        platform: DevicePlatform.macos,
        deviceClass: DeviceClass.desktop,
        osName: 'macOS',
        osVersion: '15.6',
        presence: DevicePresence.online,
      ),
    ),
    peers: peers,
    discovered: discovered,
  );

  @override
  Future<void> rescan() async {}

  @override
  Future<void> revoke(String peerId) async {}

  @override
  Future<void> setThisDeviceName(String name) async {}

  @override
  Future<void> sync({String? peerId}) async {
    if (syncError case final error?) throw error;
  }

  @override
  Future<void> unpair(String peerId) async {}
}

DeviceDetails _details({
  required String name,
  required DevicePlatform platform,
  required DeviceClass deviceClass,
  required String osName,
  required String osVersion,
  required DevicePresence presence,
  Duration? latency,
  String? endpoint,
}) {
  final observedAt = DateTime.utc(2026, 10, 3, 12);
  return DeviceDetails(
    profile: DeviceProfile(
      displayName: name,
      appVersion: '1.0.0',
      protocolVersion: 1,
      platform: platform,
      deviceClass: deviceClass,
      osName: osName,
      osVersion: osVersion,
      provenance: DeviceObservationProvenance.selfReported,
      trust: DeviceObservationTrust.authenticated,
      observedAt: observedAt,
      freshUntil: observedAt.add(const Duration(hours: 1)),
    ),
    endpoint: endpoint == null
        ? null
        : DeviceEndpoint(
            lanEndpoint: endpoint,
            provenance: DeviceObservationProvenance.observed,
            trust: DeviceObservationTrust.authenticated,
            observedAt: observedAt,
            freshUntil: observedAt.add(const Duration(hours: 1)),
          ),
    latency: latency == null
        ? null
        : DeviceLatency(
            roundTripLatency: latency,
            provenance: DeviceObservationProvenance.measured,
            trust: DeviceObservationTrust.authenticated,
            observedAt: observedAt,
            freshUntil: DateTime.utc(2026, 10, 5),
          ),
    presence: DevicePresenceObservation(
      state: presence,
      lastSeen: observedAt,
      provenance: DeviceObservationProvenance.measured,
      trust: DeviceObservationTrust.authenticated,
      observedAt: observedAt,
      freshUntil: DateTime.utc(2026, 10, 5),
    ),
  );
}

class _ScreenSession implements DevicesPairingSession {
  int revealInviteCalls = 0;

  @override
  PairingCeremony get ceremony =>
      const PairingCeremony(state: PairingState.waitingForPeer);

  @override
  Stream<PairingCeremony> get updates => const Stream<PairingCeremony>.empty();

  @override
  Future<void> cancel() async {}

  @override
  Future<void> confirm({required bool accept}) async {}

  @override
  Future<void> dispose() async {}

  @override
  Future<Uint8List> revealInviteQr() async {
    revealInviteCalls += 1;
    return _testPng();
  }

  @override
  Future<String> revealSas() async => '123456';
}

Uint8List _testPng() => base64Decode(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=',
);

class _CaptureProtection implements PairingCaptureProtection {
  @override
  Future<bool> setEnabled(bool enabled) async => true;
}
