import 'dart:async';

import 'package:copypaste_flutter/app/navigation/navigation.dart';
import 'package:copypaste_flutter/app/shell/app_shell.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/shared/inspector_table.dart';
import 'package:copypaste_flutter/app/theme/app_tokens.dart';
import 'package:copypaste_flutter/features/devices/devices.dart';
import 'package:copypaste_flutter/features/devices/device_label.dart';
import 'package:copypaste_flutter/features/settings/controller/settings_controller.dart';
import 'package:copypaste_flutter/features/settings/models/settings_models.dart';
import 'package:copypaste_flutter/features/settings/models/sync_status.dart';
import 'package:copypaste_flutter/features/settings/view/sync_header_action.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'settings_test_support.dart';

void main() {
  for (final layout in [
    (
      name: 'macOS title bar',
      platform: TargetPlatform.macOS,
      unified: true,
      size: const Size(1200, 720),
    ),
    (
      name: 'Windows header',
      platform: TargetPlatform.windows,
      unified: false,
      size: const Size(1200, 720),
    ),
    (
      name: 'Android header',
      platform: TargetPlatform.android,
      unified: false,
      size: const Size(390, 844),
    ),
  ]) {
    testWidgets('sync drawer opens and closes from ${layout.name}', (
      tester,
    ) async {
      await tester.binding.setSurfaceSize(layout.size);
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final controller = _controller(FakeSettingsRepository());
      final navigation = AppNavigationController();
      addTearDown(controller.dispose);
      addTearDown(navigation.dispose);
      await controller.initialize();
      navigation.selectDestination(AppDestination.settings);
      final visibility = <bool>[];
      await tester.pumpWidget(
        _app(
          controller,
          navigation: navigation,
          unifiedTitleBar: layout.unified,
          onVisibility: (open) {
            visibility.add(open);
            navigation.setBottomOverlayOpen(open);
          },
        ),
      );

      final open = find.byKey(const ValueKey<String>('sync-header-open'));
      final drawer = find.byKey(const ValueKey<String>('sync-details-drawer'));
      await tester.tap(open);
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      expect(drawer, findsOneWidget);
      expect(navigation.bottomOverlayOpen, isTrue);
      final drawerRect = tester.getRect(drawer);
      if (layout.size.width >= 800) {
        expect(drawerRect.width, closeTo(AppOverlaySize.drawerPanelWidth, 2));
        expect(drawerRect.right, closeTo(layout.size.width, 2));
        final bodyRect = tester.getRect(
          find.byKey(const ValueKey('app-body-drawer-overlay')),
        );
        expect(drawerRect.top, closeTo(bodyRect.top, 2));
        expect(drawerRect.height, closeTo(bodyRect.height, 2));
        expect(open.hitTestable(), findsOneWidget);
      } else {
        expect(drawerRect.width, closeTo(layout.size.width, 2));
        expect(drawerRect.bottom, closeTo(layout.size.height, 2));
      }

      await tester.tap(
        find.byKey(const ValueKey<String>('close-sync-details')),
      );
      await tester.pumpAndSettle();
      expect(drawer, findsNothing);
      expect(navigation.bottomOverlayOpen, isFalse);

      await tester.tap(open);
      await tester.pumpAndSettle();
      expect(drawer, findsOneWidget);
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      expect(drawer, findsNothing);
      expect(navigation.bottomOverlayOpen, isFalse);
      expect(visibility, [true, false, true, false]);
    }, variant: TargetPlatformVariant.only(layout.platform));
  }

  testWidgets('header reflects backend sync phases and rejects stale updates', (
    tester,
  ) async {
    final repository = FakeSettingsRepository();
    final controller = _controller(repository);
    addTearDown(controller.dispose);
    await controller.initialize();
    await tester.pumpWidget(_app(controller));
    var revision = 10;
    for (final phase in [
      SyncPhase.waiting,
      SyncPhase.syncing,
      SyncPhase.synced,
      SyncPhase.failed,
      SyncPhase.disabled,
    ]) {
      revision++;
      repository.syncChanges.add(SyncStatus(revision: revision, phase: phase));
      await tester.pumpAndSettle();
      final buttonContext = tester.element(
        find.byKey(const ValueKey<String>('sync-header-open')),
      );
      final icon = tester.widget<Icon>(find.byIcon(LucideIcons.network));
      final tone = switch (phase) {
        SyncPhase.syncing => AppStatusTone.info,
        SyncPhase.synced => AppStatusTone.success,
        SyncPhase.failed => AppStatusTone.error,
        _ => AppStatusTone.muted,
      };
      expect(icon.color, AppTheme.statusColor(buttonContext, tone));
      expect(controller.syncStatus.phase, phase);
    }
    repository.syncChanges.add(
      const SyncStatus(revision: 1, phase: SyncPhase.synced),
    );
    await tester.pumpAndSettle();
    expect(controller.syncStatus.phase, SyncPhase.disabled);
  });

  testWidgets(
    'pending items keep the completed phase and show an item warning',
    (tester) async {
      final repository = FakeSettingsRepository()
        ..capture = const CaptureSettingsState(
          running: true,
          paused: false,
          epoch: 0,
          syncStatus: SyncStatus(
            revision: 1,
            phase: SyncPhase.synced,
            peers: [
              PeerSyncStatus(
                id: 'phone',
                name: 'Android phone',
                phase: SyncPhase.synced,
                error: '5 items remain pending synchronization.',
              ),
            ],
          ),
        );
      final controller = _controller(repository);
      addTearDown(controller.dispose);
      await controller.initialize();
      await tester.pumpWidget(_app(controller));
      await tester.tap(find.byKey(const ValueKey<String>('sync-header-open')));
      await tester.pumpAndSettle();
      expect(controller.syncStatus.phase, SyncPhase.synced);
      expect(find.text('Items pending synchronization'), findsOneWidget);
      expect(
        find.text('5 items remain pending synchronization.'),
        findsOneWidget,
      );
      expect(find.text('Last sync problem'), findsNothing);
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant({
      TargetPlatform.macOS,
      TargetPlatform.windows,
      TargetPlatform.android,
    }),
  );

  testWidgets(
    'drawer shows active devices and offline peers without stale errors',
    (tester) async {
      final now = DateTime(2026, 10, 7, 20);
      final repository = FakeSettingsRepository()
        ..capture = const CaptureSettingsState(
          running: true,
          paused: false,
          epoch: 0,
          syncStatus: SyncStatus(
            revision: 1,
            phase: SyncPhase.syncing,
            peers: [
              PeerSyncStatus(
                id: 'phone',
                name: 'Android phone',
                phase: SyncPhase.syncing,
              ),
              PeerSyncStatus(
                id: 'windows',
                name: 'Windows desktop',
                phase: SyncPhase.failed,
                error: 'The device stopped responding.',
              ),
            ],
          ),
        );
      final controller = _controller(repository);
      final devices = DevicesController(
        gateway: _Devices(now),
        captureProtection: _Protection(),
        now: () => now,
      );
      addTearDown(controller.dispose);

      await controller.initialize();
      final visibility = <bool>[];
      await tester.pumpWidget(
        _app(controller, devices: devices, onVisibility: visibility.add),
      );
      await tester.tap(find.byKey(const ValueKey<String>('sync-header-open')));
      await tester.pumpAndSettle();
      expect(
        find.byKey(const ValueKey<String>('sync-details-drawer')),
        findsOneWidget,
      );
      final phoneLabel = find.byWidgetPredicate(
        (widget) => widget is DeviceLabel && widget.name == 'Android phone',
      );
      expect(phoneLabel, findsOneWidget);
      expect(
        tester.widget<DeviceLabel>(phoneLabel).deviceClass,
        DeviceClass.phone,
      );
      expect(
        find.descendant(
          of: phoneLabel,
          matching: find.byIcon(LucideIcons.smartphone),
        ),
        findsOneWidget,
      );
      expect(find.text('17 ms'), findsOneWidget);
      expect(find.byType(InspectorTable), findsWidgets);
      expect(find.text('Clips sent'), findsNothing);
      expect(find.text('Clips received'), findsNothing);
      expect(find.text('192.168.1.2:47654'), findsOneWidget);
      final desktopLabel = find.byWidgetPredicate(
        (widget) => widget is DeviceLabel && widget.name == 'Windows desktop',
      );
      await tester.scrollUntilVisible(
        desktopLabel,
        200,
        scrollable: find
            .descendant(
              of: find.byKey(const ValueKey('sync-details-drawer')),
              matching: find.byType(Scrollable),
            )
            .first,
      );
      expect(desktopLabel, findsOneWidget);
      expect(
        tester.widget<DeviceLabel>(desktopLabel).deviceClass,
        DeviceClass.desktop,
      );
      expect(
        find.descendant(
          of: desktopLabel,
          matching: find.byIcon(LucideIcons.monitor),
        ),
        findsOneWidget,
      );
      expect(find.text('Offline'), findsOneWidget);
      expect(find.text('Waiting for sync'), findsOneWidget);
      expect(find.text('Sync failed'), findsNothing);
      expect(find.text('Last sync problem'), findsNothing);
      expect(find.text('The device stopped responding.'), findsNothing);
      expect(find.text('Syncing'), findsWidgets);
      expect(tester.takeException(), isNull);
      await tester.tap(
        find.byKey(const ValueKey<String>('close-sync-details')),
      );
      await tester.pumpAndSettle();
      devices.dispose();
      expect(visibility, [true, false]);
      expect(
        find.byKey(const ValueKey<String>('sync-details-drawer')),
        findsNothing,
      );
    },
    variant: TargetPlatformVariant({
      TargetPlatform.macOS,
      TargetPlatform.windows,
      TargetPlatform.android,
    }),
  );

  testWidgets('offline failure does not make the header or drawer an error', (
    tester,
  ) async {
    final now = DateTime(2026, 10, 21, 22, 29);
    final repository = FakeSettingsRepository()
      ..capture = CaptureSettingsState(
        running: true,
        paused: false,
        epoch: 0,
        syncStatus: SyncStatus(
          revision: 1,
          phase: SyncPhase.failed,
          peers: [
            PeerSyncStatus(
              id: 'windows',
              name: 'Windows desktop',
              phase: SyncPhase.failed,
              lastSuccess: now,
              error: 'the other device did not accept this pairing code',
            ),
          ],
        ),
      );
    final controller = _controller(repository);
    final gateway = _Devices(now);
    final devices = DevicesController(
      gateway: gateway,
      captureProtection: _Protection(),
      now: () => now,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await devices.start();
    await tester.pumpWidget(_app(controller, devices: devices));
    final header = find.byKey(const ValueKey<String>('sync-header-open'));
    final semantics = tester.ensureSemantics();
    expect(
      find.bySemanticsLabel('Synchronization: Waiting for sync'),
      findsOneWidget,
    );
    semantics.dispose();
    final icon = tester.widget<Icon>(find.byIcon(LucideIcons.network));
    expect(
      icon.color,
      AppTheme.statusColor(tester.element(header), AppStatusTone.muted),
    );
    await tester.tap(header);
    await tester.pumpAndSettle();
    expect(find.text('Offline'), findsOneWidget);
    expect(find.text('Sync failed'), findsNothing);
    expect(find.text('Last sync problem'), findsNothing);
    expect(find.text('Last successful sync'), findsOneWidget);
    expect(controller.syncStatus.peers.single.error, isNotNull);

    gateway.windowsOnline = true;
    await devices.refresh();
    await tester.pumpAndSettle();
    expect(find.text('Available'), findsOneWidget);
    expect(find.text('Sync failed'), findsWidgets);
    expect(find.text('Last sync problem'), findsOneWidget);
    expect(
      find.text('the other device did not accept this pairing code'),
      findsOneWidget,
    );
    expect(tester.takeException(), isNull);
    await tester.tap(find.byKey(const ValueKey<String>('close-sync-details')));
    await tester.pumpAndSettle();
    devices.dispose();
  });

  testWidgets('sync details fit a narrow phone with enlarged text', (
    tester,
  ) async {
    addTearDown(() => tester.binding.setSurfaceSize(null));
    addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
    await tester.binding.setSurfaceSize(const Size(320, 640));
    tester.platformDispatcher.textScaleFactorTestValue = 2;
    final repository = FakeSettingsRepository()
      ..capture = const CaptureSettingsState(
        running: true,
        paused: false,
        epoch: 0,
        syncStatus: SyncStatus(
          phase: SyncPhase.failed,
          peers: [
            PeerSyncStatus(
              id: 'phone',
              name: 'Android phone',
              phase: SyncPhase.failed,
              error: 'The device stopped responding.',
            ),
          ],
        ),
      );
    final controller = _controller(repository);
    addTearDown(controller.dispose);
    await controller.initialize();
    await tester.pumpWidget(_app(controller));
    await tester.tap(find.byKey(const ValueKey<String>('sync-header-open')));
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
    await tester.tap(find.byKey(const ValueKey<String>('close-sync-details')));
    await tester.pumpAndSettle();
  });
}

SettingsController _controller(FakeSettingsRepository repository) =>
    SettingsController(
      repository: repository,
      filePicker: FakeSettingsFilePicker(),
      screenshotProtection: FakeScreenshotProtection(),
      notifications: FakeCaptureNotificationPort(),
      captureRefreshInterval: Duration.zero,
    );

Widget _app(
  SettingsController controller, {
  DevicesController? devices,
  ValueChanged<bool>? onVisibility,
  AppNavigationController? navigation,
  bool unifiedTitleBar = false,
}) => ShadcnApp(
  theme: AppTheme.light,
  builder: AppTheme.builder,
  home: navigation != null
      ? AppShell(
          controller: navigation,
          unifiedTitleBar: unifiedTitleBar,
          headerActions: {
            AppDestination.settings: [
              SyncHeaderAction(
                controller: controller,
                devices: devices,
                onDrawerVisibilityChanged: onVisibility,
              ),
            ],
          },
          destinations: const {
            AppDestination.history: Text('History body'),
            AppDestination.devices: Text('Devices body'),
            AppDestination.settings: Text('Settings body'),
          },
        )
      : Scaffold(
          child: Center(
            child: SyncHeaderAction(
              controller: controller,
              devices: devices,
              onDrawerVisibilityChanged: onVisibility,
            ),
          ),
        ),
);

class _Devices implements DevicesGateway {
  _Devices(this.now);
  final DateTime now;
  bool windowsOnline = false;
  @override
  Stream<void> get changes => const Stream.empty();
  @override
  Future<DevicesSnapshot> load() async => DevicesSnapshot(
    thisDevice: const ThisDevice(
      name: 'Mac',
      appVersion: '1.0.6',
      protocolVersion: 6,
    ),
    discovered: const [],
    peers: [
      DevicePeer(
        id: 'phone',
        name: 'Android phone',
        lastSeen: now,
        online: true,
        details: DeviceDetails(
          profile: DeviceProfile(
            displayName: 'Android phone',
            platform: DevicePlatform.android,
            deviceClass: DeviceClass.phone,
            provenance: DeviceObservationProvenance.selfReported,
            trust: DeviceObservationTrust.authenticated,
            observedAt: now,
          ),
          endpoint: DeviceEndpoint(
            lanEndpoint: '192.168.1.2:47654',
            provenance: DeviceObservationProvenance.observed,
            trust: DeviceObservationTrust.authenticated,
            observedAt: now,
          ),
          latency: DeviceLatency(
            roundTripLatency: const Duration(milliseconds: 17),
            provenance: DeviceObservationProvenance.measured,
            trust: DeviceObservationTrust.authenticated,
            observedAt: now,
            freshUntil: now.add(const Duration(minutes: 1)),
          ),
        ),
      ),
      DevicePeer(
        id: 'windows',
        name: 'Windows desktop',
        lastSeen: now,
        online: windowsOnline,
        details: DeviceDetails(
          presence: DevicePresenceObservation(
            state: windowsOnline
                ? DevicePresence.online
                : DevicePresence.offline,
            lastSeen: now,
            provenance: DeviceObservationProvenance.measured,
            trust: DeviceObservationTrust.authenticated,
            observedAt: now,
            freshUntil: now.add(const Duration(minutes: 1)),
          ),
          profile: DeviceProfile(
            displayName: 'Windows desktop',
            platform: DevicePlatform.windows,
            deviceClass: DeviceClass.desktop,
            provenance: DeviceObservationProvenance.selfReported,
            trust: DeviceObservationTrust.authenticated,
            observedAt: now,
          ),
        ),
      ),
    ],
  );
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Protection implements PairingCaptureProtection {
  @override
  Future<bool> setEnabled(bool enabled) async => true;
}
