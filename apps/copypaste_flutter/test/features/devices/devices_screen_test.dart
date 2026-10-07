import 'dart:async';
import 'dart:convert';

import 'package:copypaste_flutter/features/devices/devices.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  for (final size in [const Size(320, 640), const Size(1400, 900)]) {
    testWidgets(
      'keeps nearby discovery compact and excludes paired devices at $size',
      (tester) async {
        final discovered = [
          _discoveredDevice('Paired tablet', paired: true),
          _discoveredDevice('Nearby laptop'),
        ];
        final gateway = _ScreenGateway(discovered: discovered);
        final controller = DevicesController(
          gateway: gateway,
          captureProtection: _CaptureProtection(),
        );
        addTearDown(controller.dispose);
        await _pumpDevices(tester, controller, size: size);

        final section = find.byKey(const ValueKey('nearby-devices'));
        final row = find.byKey(
          const ValueKey('discovered-device-row-Nearby laptop'),
        );
        final scan = find.byKey(const ValueKey('rescan-devices'));
        expect(find.text('Paired tablet'), findsNothing);
        expect(find.text('Already paired'), findsNothing);
        expect(find.text('Found on this network'), findsNothing);
        expect(row, findsOneWidget);
        expect(
          find.descendant(of: row, matching: find.text('Windows 11')),
          findsOneWidget,
        );
        expect(tester.widget(row), isA<Basic>());
        expect(
          find.descendant(of: section, matching: find.byType(Divider)),
          findsNothing,
        );
        final scanButton = tester.widget<Button>(scan);
        final scanContext = tester.element(scan);
        expect(
          scanButton.style.padding(scanContext, const {}),
          EdgeInsets.zero,
        );
        final decoration = scanButton.style.decoration(scanContext, const {});
        expect(decoration, isA<BoxDecoration>());
        expect((decoration as BoxDecoration).border, isNull);
        expect(decoration.color, anyOf(isNull, Colors.transparent));
        expect(
          tester.getCenter(scan).dy,
          closeTo(tester.getCenter(find.text('Nearby devices')).dy, 1),
        );

        final pair = find.descendant(of: row, matching: find.text('Pair'));
        await tester.ensureVisible(pair);
        await tester.tap(pair);
        await tester.pumpAndSettle();
        expect(controller.pairingEntryMode, PairingEntryMode.enterCode);
        expect(controller.pendingAddress, '192.0.2.30:47654');
        await tester.runAsync(controller.closePairing);
        await tester.pumpAndSettle();

        discovered[1] = _discoveredDevice('Nearby laptop', paired: true);
        await controller.refresh();
        await tester.pumpAndSettle();
        expect(find.text('Nearby laptop'), findsNothing);
        expect(find.text('No devices nearby'), findsOneWidget);
        expect(scan, findsOneWidget);
        expect(find.text('Rescan'), findsNothing);
        expect(tester.takeException(), isNull);
      },
      variant: TargetPlatformVariant({
        TargetPlatform.android,
        TargetPlatform.macOS,
        TargetPlatform.windows,
      }),
    );
  }

  testWidgets('scans from the nearby header and restores it after failure', (
    tester,
  ) async {
    final gateway = _ScreenGateway(discovered: const []);
    final controller = DevicesController(
      gateway: gateway,
      captureProtection: _CaptureProtection(),
    );
    addTearDown(controller.dispose);
    await _pumpDevices(tester, controller);
    final pending = Completer<void>();
    gateway.rescanPending = pending;
    final scan = find.byKey(const ValueKey('rescan-devices'));
    await tester.tap(scan);
    await tester.pump();
    expect(gateway.rescanCalls, 1);
    expect(controller.rescanInFlight, isTrue);
    expect(find.text('Scanning…'), findsOneWidget);
    expect(tester.widget<Button>(scan).onPressed, isNull);
    expect(find.text('No devices nearby'), findsOneWidget);
    pending.completeError(StateError('Discovery unavailable'));
    await tester.pumpAndSettle();
    expect(controller.rescanInFlight, isFalse);
    expect(find.text('Scan'), findsOneWidget);
    expect(tester.widget<Button>(scan).onPressed, isNotNull);
    expect(find.textContaining('Discovery unavailable'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
    'keeps nearby rows and scanning usable with enlarged text at 320 pixels',
    (tester) async {
      tester.platformDispatcher.textScaleFactorTestValue = 2;
      addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
      final gateway = _ScreenGateway(
        peers: const [],
        discovered: [
          _discoveredDevice('A nearby laptop with a long device name'),
          _discoveredDevice('Another laptop'),
        ],
      );
      final controller = DevicesController(
        gateway: gateway,
        captureProtection: _CaptureProtection(),
      );
      addTearDown(controller.dispose);
      await _pumpDevices(tester, controller, size: const Size(320, 640));
      expect(tester.takeException(), isNull);
      final nearby = find.byKey(const ValueKey('nearby-devices'));
      expect(
        find.descendant(of: nearby, matching: find.byType(Divider)),
        findsOneWidget,
      );
      final scan = find.byKey(const ValueKey('rescan-devices'));
      final pending = Completer<void>();
      gateway.rescanPending = pending;
      await tester.ensureVisible(scan);
      await tester.pumpAndSettle();
      await tester.tap(scan);
      await tester.pump();
      expect(find.text('Scanning…'), findsOneWidget);
      expect(tester.takeException(), isNull);
      pending.complete();
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant({
      TargetPlatform.android,
      TargetPlatform.macOS,
      TargetPlatform.windows,
    }),
  );

  for (final size in [const Size(320, 640), const Size(1400, 900)]) {
    testWidgets(
      'shows selectable invitation details below QR at $size',
      (tester) async {
        final controller = DevicesController(
          gateway: _ScreenGateway(),
          captureProtection: _CaptureProtection(),
        );
        addTearDown(controller.dispose);
        await _pumpDevices(tester, controller, size: size);
        await tester.runAsync(controller.openInvitation);
        await tester.pumpAndSettle();
        final qr = find.byKey(const ValueKey('pairing-invite-qr'));
        final code = find.byKey(const ValueKey('pairing-invite-code'));
        final address = find.byKey(const ValueKey('pairing-invite-address'));
        expect(qr, findsOneWidget);
        expect(
          tester.widget<SelectableText>(code).data,
          controller.invitation!.code,
        );
        expect(
          tester.widget<SelectableText>(address).data,
          '192.168.50.232:62951',
        );
        expect(
          tester.getTopLeft(code).dy,
          greaterThan(tester.getBottomLeft(qr).dy),
        );
        expect(
          tester.getTopLeft(address).dy,
          greaterThan(tester.getTopLeft(code).dy),
        );
        expect(tester.takeException(), isNull);
        await tester.runAsync(controller.closePairing);
        await tester.pumpAndSettle();
        expect(code, findsNothing);
        expect(address, findsNothing);
      },
      variant: TargetPlatformVariant({
        TargetPlatform.android,
        TargetPlatform.macOS,
        TargetPlatform.windows,
      }),
    );
  }

  for (final accept in [true, false]) {
    testWidgets(
      'shows the joined SAS immediately with Accept and Reject: $accept',
      (tester) async {
        final gateway = _ScreenGateway();
        gateway.session.ceremony = const PairingCeremony(
          state: PairingState.awaitingConfirmation,
        );
        final controller = DevicesController(
          gateway: gateway,
          captureProtection: _CaptureProtection(),
        );
        addTearDown(controller.dispose);
        await _pumpDevices(tester, controller);
        await tester.runAsync(
          () =>
              controller.joinPairingUri('copypaste://pair/v1?test=invitation'),
        );
        await tester.pumpAndSettle();
        expect(find.text('123456'), findsOneWidget);
        expect(find.text('Show verification code'), findsNothing);
        expect(find.text('Accept'), findsOneWidget);
        expect(find.text('Reject'), findsOneWidget);
        expect(gateway.session.confirmations, isEmpty);
        await tester.tap(find.text(accept ? 'Accept' : 'Reject'));
        await tester.pumpAndSettle();
        expect(gateway.session.confirmations, [accept]);
        await tester.runAsync(controller.closePairing);
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
      },
      variant: TargetPlatformVariant({
        TargetPlatform.android,
        TargetPlatform.macOS,
        TargetPlatform.windows,
      }),
    );
  }

  for (final platform in [TargetPlatform.macOS, TargetPlatform.windows]) {
    for (final size in [const Size(390, 844), const Size(1400, 900)]) {
      testWidgets('keeps the embedded QR scanner on $platform at $size', (
        tester,
      ) async {
        final controller = DevicesController(
          gateway: _ScreenGateway(),
          captureProtection: _CaptureProtection(),
        );
        addTearDown(controller.dispose);
        await _pumpDevices(tester, controller, size: size);
        await tester.tap(find.byKey(const ValueKey('scan-pairing-qr')));
        await tester.pumpAndSettle();
        expect(controller.usesSystemScanner, isFalse);
        expect(controller.pairingInspectorOpen, isTrue);
        expect(
          find.byKey(
            ValueKey(
              size.width < 800
                  ? 'devices-pairing-drawer'
                  : 'devices-pairing-inspector',
            ),
          ),
          findsOneWidget,
        );
        expect(
          find.byKey(const ValueKey('start-pairing-scanner')),
          findsOneWidget,
        );
        await controller.closePairing();
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
      }, variant: TargetPlatformVariant({platform}));
    }
  }

  for (final size in [const Size(390, 844), const Size(1000, 900)]) {
    for (final outcome in [
      'cancel',
      'success',
      'unavailable',
      'invalid',
      'join failure',
    ]) {
      testWidgets('system QR scanner $outcome at $size', (tester) async {
        const channel = MethodChannel('com.copypaste.app/qr_scanner');
        final result = Completer<String?>();
        var scanCalls = 0;
        tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
          channel,
          (call) async {
            if (call.method == 'scan') {
              scanCalls++;
              return result.future;
            }
            return null;
          },
        );
        addTearDown(
          () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
            channel,
            null,
          ),
        );
        final gateway = _ScreenGateway();
        if (outcome == 'join failure') {
          gateway.joinError = StateError('Join failed');
        }
        final controller = DevicesController(
          gateway: gateway,
          captureProtection: _CaptureProtection(),
        );
        addTearDown(controller.dispose);
        await _pumpDevices(tester, controller, size: size);
        await tester.tap(find.byKey(const ValueKey('scan-pairing-qr')));
        await tester.pumpAndSettle();

        expect(scanCalls, 1);
        expect(controller.systemScanInFlight, isTrue);
        expect(
          find.byKey(const ValueKey('devices-pairing-drawer')),
          findsNothing,
        );
        expect(
          find.byKey(const ValueKey('devices-pairing-inspector')),
          findsNothing,
        );
        expect(find.text('Opening Google scanner…'), findsNothing);

        switch (outcome) {
          case 'success':
          case 'join failure':
            result.complete('copypaste://pair/v1?test=invitation');
          case 'unavailable':
            result.completeError(
              PlatformException(code: 'scanner_unavailable'),
            );
          case 'invalid':
            result.completeError(PlatformException(code: 'invalid_pairing_qr'));
          default:
            result.complete(null);
        }
        await tester.pumpAndSettle();

        expect(controller.systemScanInFlight, isFalse);
        if (outcome == 'success' || outcome == 'join failure') {
          expect(
            find.byKey(
              ValueKey(
                size.width < 800
                    ? 'devices-pairing-drawer'
                    : 'devices-pairing-inspector',
              ),
            ),
            findsOneWidget,
          );
          if (outcome == 'join failure') {
            expect(find.text('Pairing action failed'), findsOneWidget);
            expect(find.text('Connecting device…'), findsNothing);
          }
          await tester.runAsync(controller.closePairing);
          await tester.pumpAndSettle();
        } else {
          expect(controller.pairingEntryMode, isNull);
          expect(
            find.byKey(const ValueKey('devices-pairing-drawer')),
            findsNothing,
          );
          expect(
            find.byKey(const ValueKey('devices-pairing-inspector')),
            findsNothing,
          );
          if (outcome == 'cancel') {
            expect(controller.errorMessage, isNull);
            expect(find.byType(Alert), findsNothing);
            await tester.tap(find.byKey(const ValueKey('this-device-card')));
            await tester.pumpAndSettle();
            expect(controller.deviceDetailsOpen, isTrue);
            controller.closeDeviceDetails();
            await tester.pumpAndSettle();
          } else {
            expect(find.text('Scanner unavailable'), findsOneWidget);
            expect(controller.canChangePairingMode, isTrue);
          }
        }
        expect(tester.takeException(), isNull);
      }, variant: TargetPlatformVariant({TargetPlatform.android}));
    }
  }

  for (final (platform, size) in [
    (TargetPlatform.macOS, const Size(1000, 900)),
    (TargetPlatform.windows, const Size(1400, 900)),
  ]) {
    testWidgets(
      'switches header actions within the open inspector on $platform',
      (tester) async {
        final gateway = _ScreenGateway(peers: const [], discovered: const []);
        final controller = DevicesController(
          gateway: gateway,
          captureProtection: _CaptureProtection(),
        );
        expect(controller.usesSystemScanner, isFalse);
        addTearDown(controller.dispose);
        await _pumpDevices(tester, controller, size: size);

        for (final (action, content) in [
          ('enter-pairing-code', 'pairing-code-input'),
          ('scan-pairing-qr', 'start-pairing-scanner'),
          ('pair-device', 'pairing-invite-qr'),
          ('enter-pairing-code', 'pairing-code-input'),
        ]) {
          final command = tester
              .widget<Button>(find.byKey(ValueKey<String>(action)))
              .onPressed;
          expect(command, isNotNull);
          await tester.runAsync(command as Future<void> Function());
          await tester.pumpAndSettle();
          expect(
            find.byKey(ValueKey<String>(content)),
            findsOneWidget,
            reason:
                '$action: mode=${controller.pairingEntryMode}, '
                'enabled=${controller.canChangePairingMode}, '
                'error=${controller.errorMessage}',
          );
          expect(
            find.byKey(const ValueKey<String>('devices-pairing-inspector')),
            findsOneWidget,
          );
          for (final key in [
            'pair-device',
            'scan-pairing-qr',
            'enter-pairing-code',
          ]) {
            expect(
              tester
                  .widget<Button>(find.byKey(ValueKey<String>(key)))
                  .onPressed,
              isNotNull,
              reason: '$key after $action',
            );
          }
          expect(tester.takeException(), isNull);
        }
        expect(gateway.session.cancelCalls, 1);
      },
      variant: TargetPlatformVariant({platform}),
    );
  }

  testWidgets(
    'refreshes cannot restart drawer dismissal or leave a touch barrier',
    (tester) async {
      final controller = DevicesController(
        gateway: _ScreenGateway(),
        captureProtection: _CaptureProtection(),
      );
      addTearDown(controller.dispose);
      await _pumpDevices(tester, controller, size: const Size(390, 844));
      await tester.tap(find.byKey(const ValueKey<String>('this-device-card')));
      await tester.pumpAndSettle();
      controller.closeDeviceDetails();
      for (var frame = 0; frame < 10; frame++) {
        await controller.refresh();
        await tester.pump(const Duration(milliseconds: 60));
      }
      expect(
        find.byKey(const ValueKey<String>('devices-details-drawer')),
        findsNothing,
      );
      await tester.tap(find.byKey(const ValueKey<String>('this-device-card')));
      await tester.pumpAndSettle();
      expect(
        find.byKey(const ValueKey<String>('devices-details-drawer')),
        findsOneWidget,
      );
      expect(tester.takeException(), isNull);
    },
  );
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
    for (final name in ['Desktop', 'Phone']) {
      expect(find.text(name), findsOneWidget);
    }
    expect(find.text('Desktop · macOS 15.6'), findsOneWidget);
    expect(find.text('Phone · Android 16'), findsOneWidget);
    expect(find.text('This device'), findsNothing);
    expect(find.text('Trusted'), findsNothing);
    expect(find.text('Available'), findsNothing);
    expect(find.text('24 ms'), findsNothing);
    expect(find.text('Tablet'), findsNothing);
    expect(find.text('Already paired'), findsNothing);
    expect(find.text('No devices nearby'), findsOneWidget);
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

    expect(
      find.byKey(const ValueKey<String>('rescan-devices')),
      findsOneWidget,
    );
    expect(find.text('Ready to pair'), findsNothing);
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
    expect(find.text('No devices nearby'), findsOneWidget);
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
    variant: TargetPlatformVariant({
      TargetPlatform.macOS,
      TargetPlatform.windows,
    }),
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
      theme: AppTheme.light,
      builder: AppTheme.builder,
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
              details: _details(
                name: 'Tablet',
                platform: DevicePlatform.android,
                deviceClass: DeviceClass.tablet,
                osName: 'Android',
                osVersion: '16',
                presence: DevicePresence.online,
              ),
            ),
          ];

  final _ScreenSession session = _ScreenSession();
  final List<DevicePeer> peers;
  final List<DiscoveredDevice> discovered;
  Completer<void>? rescanPending;
  int rescanCalls = 0;
  Object? syncError;
  Object? joinError;

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
  Future<DevicesPairingSession> joinPairingUri(String uri) async {
    if (joinError case final error?) throw error;
    return session;
  }

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
  Future<void> rescan() async {
    rescanCalls++;
    await rescanPending?.future;
  }

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

DiscoveredDevice _discoveredDevice(String name, {bool paired = false}) =>
    DiscoveredDevice(
      id: name,
      name: name,
      address: '192.0.2.30:47654',
      paired: paired,
      lastSeen: DateTime.utc(2026, 10, 3),
      details: _details(
        name: name,
        platform: DevicePlatform.windows,
        deviceClass: DeviceClass.laptop,
        osName: 'Windows',
        osVersion: '11',
        presence: DevicePresence.online,
      ),
    );

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
  final List<bool> confirmations = [];
  int revealInviteCalls = 0;
  int cancelCalls = 0;

  @override
  PairingCeremony ceremony = const PairingCeremony(
    state: PairingState.waitingForPeer,
  );

  @override
  Stream<PairingCeremony> get updates => const Stream<PairingCeremony>.empty();

  @override
  Future<void> cancel() async {
    cancelCalls += 1;
  }

  @override
  Future<void> confirm({required bool accept}) async {
    confirmations.add(accept);
  }

  @override
  Future<void> dispose() async {}

  @override
  Future<PairingInvitation> revealInvitation() async {
    revealInviteCalls += 1;
    return PairingInvitation(
      qrPng: _testPng(),
      code: 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567ABCDEFGHIJKLMNOPQRST',
      address: '192.168.50.232:62951',
    );
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
