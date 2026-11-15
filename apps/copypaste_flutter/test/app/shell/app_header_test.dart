import 'package:copypaste_flutter/app/navigation/navigation.dart';
import 'package:copypaste_flutter/app/shell/shell.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/app/theme/app_tokens.dart';
import 'package:copypaste_flutter/features/devices/devices_gateway.dart';
import 'package:copypaste_flutter/features/settings/controller/settings_controller.dart';
import 'package:copypaste_flutter/features/settings/models/settings_models.dart';
import 'package:copypaste_flutter/features/settings/models/storage_usage_text.dart';
import 'package:copypaste_flutter/features/settings/models/sync_status.dart';
import 'package:copypaste_flutter/features/settings/view/capture_header_action.dart';
import 'package:copypaste_flutter/features/settings/view/storage_header_action.dart';
import 'package:copypaste_flutter/features/settings/view/sync_header_action.dart';
import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../features/settings/settings_test_support.dart';

const _entryPoints = ['notifications', 'transfers', 'sync-header-open'];
const _deviceCommands = [
  ('pair-device', 'Add device'),
  ('scan-pairing-qr', 'Scan QR'),
  ('enter-pairing-code', 'Enter code'),
];

void main() {
  late SettingsController settings;
  late AppNavigationController navigation;
  late ValueNotifier<StorageUsage?> storage;
  var storageOpened = 0;

  // Stage boundaries are only meaningful with the application typeface.
  setUpAll(() async {
    final font = FontLoader('packages/shadcn_flutter/GeistSans');
    for (final face in ['Regular', 'Medium', 'SemiBold']) {
      font.addFont(
        rootBundle.load('packages/shadcn_flutter/lib/fonts/Geist-$face.otf'),
      );
    }
    await font.load();
  });

  setUp(() async {
    settings = SettingsController(
      repository: FakeSettingsRepository()
        ..capture = CaptureSettingsState(
          running: true,
          paused: false,
          epoch: 0,
          syncStatus: SyncStatus(
            phase: SyncPhase.synced,
            peers: [
              for (final name in ['Phone', 'Laptop', 'Tablet'])
                PeerSyncStatus(
                  id: name,
                  name: name,
                  phase: SyncPhase.synced,
                  connection: DeviceConnectionState.online,
                ),
            ],
          ),
        ),
      filePicker: FakeSettingsFilePicker(),
      screenshotProtection: FakeScreenshotProtection(),
      notifications: FakeCaptureNotificationPort(),
      captureRefreshInterval: Duration.zero,
    );
    await settings.initialize();
    navigation = AppNavigationController();
    storage = ValueNotifier(
      const StorageUsage(usedBytes: 2254857830, quotaBytes: 5 * _gib),
    );
    storageOpened = 0;
  });

  tearDown(() {
    navigation.dispose();
    settings.dispose();
    storage.dispose();
    AppStrings.useLocale(AppLocalization.fallback);
  });

  Future<void> pump(
    WidgetTester tester, {
    required double width,
    bool unified = false,
    Locale locale = AppLocalization.fallback,
    double textScale = 1,
    Widget history = const Text('History body'),
  }) async {
    final typography = AppTheme.light.typography;
    final theme = AppTheme.light.copyWith(
      typography: () => typography.copyWith(
        sans: () => typography.sans.copyWith(
          fontFamily: 'packages/shadcn_flutter/GeistSans',
        ),
      ),
    );
    await tester.binding.setSurfaceSize(Size(width, 720));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(
      ShadcnApp(
        theme: theme,
        builder: AppTheme.builder,
        locale: locale,
        localizationsDelegates: AppLocalization.delegates,
        supportedLocales: AppLocalization.supportedLocales,
        home: AppStringsLocale(
          child: Builder(
            builder: (context) => MediaQuery(
              data: MediaQuery.of(
                context,
              ).copyWith(textScaler: TextScaler.linear(textScale)),
              child: AppShell(
                controller: navigation,
                unifiedTitleBar: unified,
                headerActions: const {
                  AppDestination.devices: [_DeviceCommands()],
                },
                headerStatus: [
                  CaptureHeaderAction(controller: settings),
                  SyncHeaderAction(
                    controller: settings,
                    onDrawerVisibilityChanged:
                        navigation.handleDrawerVisibilityChanged,
                  ),
                  StorageHeaderAction(
                    usage: storage,
                    onPressed: () => storageOpened++,
                  ),
                ],
                headerEnd: [
                  _countedAction('transfers', LucideIcons.download, 1),
                  _countedAction('notifications', LucideIcons.bell, 2),
                ],
                destinations: {
                  AppDestination.history: history,
                  AppDestination.devices: const Text('Devices body'),
                  AppDestination.settings: const Text('Settings body'),
                },
              ),
            ),
          ),
        ),
      ),
    );
    // Button insets ease to a new text scale instead of jumping.
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 500));
  }

  HeaderStage stageOf(WidgetTester tester) =>
      HeaderFit.stageOf(tester.element(find.byType(AppHeader)));

  /// Checks what every stage promises: one navigation, every entry point
  /// inside the window, one control height, and the parts the stage keeps.
  void expectStageContract(
    WidgetTester tester, {
    required double width,
    required bool unified,
    required String reason,
  }) {
    expect(tester.takeException(), isNull, reason: reason);
    final stage = stageOf(tester);
    final headerNavigation = find
        .byKey(const ValueKey<String>('header-destination-history'))
        .hitTestable();
    final bottomNavigation = find.byKey(
      const ValueKey<String>('bottom-navigation'),
    );
    expect(
      headerNavigation.evaluate().length,
      stage.showsHeaderNavigation ? 1 : 0,
      reason: reason,
    );
    expect(
      bottomNavigation.evaluate().length,
      stage.showsHeaderNavigation ? 0 : 1,
      reason: reason,
    );

    final controls = [
      for (final key in [
        ..._entryPoints,
        'capture-header-toggle',
        // Storage gives the bottom-navigation header room to its title.
        if (stage.showsHeaderNavigation) 'storage-header-open',
        if (navigation.selectedDestination == AppDestination.devices)
          for (final (key, _) in _deviceCommands) key,
        if (stage.showsHeaderNavigation)
          for (final destination in AppDestination.values)
            'header-destination-${destination.name}',
      ])
        find.byKey(ValueKey<String>(key)),
    ];
    final height = tester.getSize(controls.first).height;
    for (final control in controls) {
      expect(control.hitTestable(), findsOneWidget, reason: '$reason $control');
      final rect = tester.getRect(control);
      expect(rect.left, greaterThanOrEqualTo(0), reason: '$reason $control');
      expect(rect.right, lessThanOrEqualTo(width), reason: '$reason $control');
      expect(rect.height, height, reason: '$reason $control');
    }

    final l10n = AppStrings.current;
    void expectPart(Finder part, bool visible, String name) => expect(
      part.hitTestable().evaluate().isNotEmpty,
      visible,
      reason: '$reason $name in ${stage.name}',
    );
    final header = find.byType(AppHeader);
    Finder inHeader(Finder finder) =>
        find.descendant(of: header, matching: finder);
    // Command labels go first, so status words never leave while they show.
    if (navigation.selectedDestination == AppDestination.devices) {
      expectPart(
        inHeader(find.text(_deviceCommands.first.$2)),
        stage == HeaderStage.detailed,
        'command labels',
      );
    }
    expectPart(
      inHeader(find.text(' · ${l10n.syncHeaderDeviceCount(3)}')),
      stage.index < HeaderStage.labeled.index,
      'secondary status text',
    );
    expectPart(
      inHeader(find.text(StorageUsageText.short(l10n, storage.value!))),
      stage.index < HeaderStage.labeled.index,
      'storage figures',
    );
    expectPart(
      inHeader(find.text(l10n.syncStatusUpToDate)),
      stage.showsStatusText,
      'sync state',
    );
    expectPart(
      inHeader(find.text(l10n.platformCaptureStateRunning)),
      stage.showsStatusText,
      'capture state',
    );
    expectPart(
      find.byKey(const ValueKey<String>('header-wordmark')),
      stage.index < HeaderStage.unbranded.index,
      'wordmark',
    );
    expectPart(
      inHeader(
        find.text(navigation.selectedDestination.navigationDestination.label),
      ),
      true,
      'destination name',
    );
    if (unified && stage.showsHeaderNavigation) {
      expect(
        tester
            .getSize(
              find.byKey(const ValueKey<String>('macos-window-drag-region')),
            )
            .width,
        greaterThanOrEqualTo(AppSpacing.xxxl),
        reason: reason,
      );
    }
  }

  testWidgets(
    'every stage fits the window and keeps one navigation and all entry points',
    (tester) async {
      final unified = defaultTargetPlatform == TargetPlatform.macOS;
      for (final locale in const [Locale('en'), Locale('de')]) {
        for (final textScale in [1.0, 2.0]) {
          for (final destination in [
            AppDestination.history,
            AppDestination.devices,
          ]) {
            navigation.selectDestination(destination);
            for (final width in [1100.0, 900.0, 760.0, 680.0, 640.0, 600.0]) {
              await pump(
                tester,
                width: width,
                unified: unified,
                locale: locale,
                textScale: textScale,
              );
              expectStageContract(
                tester,
                width: width,
                unified: unified,
                reason:
                    '$locale x$textScale ${destination.name} at $width '
                    '(${stageOf(tester).name})',
              );
            }
          }
        }
      }
    },
    variant: const TargetPlatformVariant({
      TargetPlatform.macOS,
      TargetPlatform.windows,
    }),
  );

  testWidgets(
    'narrowing gives up parts in order and widening restores the same stages',
    (tester) async {
      final unified = defaultTargetPlatform == TargetPlatform.macOS;
      navigation.selectDestination(AppDestination.devices);
      for (final textScale in [1.0, 2.0]) {
        final widths = [
          // Doubled text needs a wider window before every label fits.
          for (
            var width = 1900.0 * textScale;
            width >= 600;
            width -= 13 * textScale
          )
            width,
        ];
        final narrowing = <HeaderStage>[];
        for (final width in widths) {
          await pump(
            tester,
            width: width,
            unified: unified,
            textScale: textScale,
          );
          expectStageContract(
            tester,
            width: width,
            unified: unified,
            reason: 'x$textScale at $width',
          );
          narrowing.add(stageOf(tester));
        }
        // Stages only ever move one way while the window narrows.
        for (var index = 1; index < narrowing.length; index++) {
          expect(
            narrowing[index].index,
            greaterThanOrEqualTo(narrowing[index - 1].index),
            reason: 'x$textScale at ${widths[index]}',
          );
        }
        expect(narrowing.toSet(), HeaderStage.values.toSet());
        // A width maps to one stage whichever side it is reached from.
        for (final (index, width) in widths.indexed.toList().reversed) {
          await pump(
            tester,
            width: width,
            unified: unified,
            textScale: textScale,
          );
          expect(stageOf(tester), narrowing[index], reason: '$width');
        }
      }
    },
    variant: const TargetPlatformVariant({
      TargetPlatform.macOS,
      TargetPlatform.windows,
    }),
  );

  testWidgets(
    'status is one grouped object before transfers and notifications at the end',
    (tester) async {
      navigation.selectDestination(AppDestination.devices);
      await pump(tester, width: 1400);
      expect(stageOf(tester), HeaderStage.detailed);
      final group = find.byKey(const ValueKey<String>('header-status-group'));
      final cells = [
        for (final key in [
          'capture-header-toggle',
          'sync-header-open',
          'storage-header-open',
        ])
          find.byKey(ValueKey<String>(key)),
      ];
      final transfers = find.byKey(const ValueKey<String>('transfers'));
      final notifications = find.byKey(const ValueKey<String>('notifications'));

      void expectOneGroup(String reason) {
        expect(group, findsOneWidget, reason: reason);
        final surface = tester.widget<DecoratedBox>(group).decoration;
        expect(
          surface,
          AppTheme.headerStatusGroupDecoration(tester.element(group)),
          reason: reason,
        );
        // No other surface or outline inside the group.
        expect(
          find.descendant(
            of: group,
            matching: find.byWidgetPredicate(
              (widget) =>
                  widget is DecoratedBox &&
                  widget.decoration == surface &&
                  widget.key == null,
            ),
          ),
          findsNothing,
          reason: reason,
        );
        final groupRect = tester.getRect(group);
        for (final (index, cell) in cells.indexed) {
          expect(
            find.descendant(of: group, matching: cell),
            findsOneWidget,
            reason: reason,
          );
          final rect = tester.getRect(cell);
          expect(rect.height, groupRect.height, reason: reason);
          final rest = tester
              .widget<Button>(cell)
              .style
              .decoration(tester.element(cell), const {});
          expect((rest as BoxDecoration).color, isNull, reason: reason);
          // Cells meet at a thin divider, inside the one outline.
          if (index > 0) {
            expect(
              rect.left,
              tester.getRect(cells[index - 1]).right,
              reason: reason,
            );
            expect(
              find.ancestor(
                of: cell,
                matching: find.byWidgetPredicate(
                  (widget) =>
                      widget is DecoratedBox &&
                      widget.decoration ==
                          AppTheme.headerStatusDividerDecoration(
                            tester.element(group),
                          ),
                ),
              ),
              findsOneWidget,
              reason: reason,
            );
          }
        }
        expect(tester.getRect(cells.first).left, groupRect.left);
        expect(tester.getRect(cells.last).right, groupRect.right);
        // Commands, then the group, then transfers and notifications at the end.
        final order = [
          find.byKey(ValueKey<String>(_deviceCommands.last.$1)),
          group,
          transfers,
          notifications,
        ].map((finder) => tester.getRect(finder)).toList();
        for (var index = 1; index < order.length; index++) {
          expect(
            order[index].left,
            greaterThan(order[index - 1].right),
            reason: reason,
          );
        }
        expect(tester.getSize(transfers).height, groupRect.height);
        expect(tester.getSize(notifications).height, groupRect.height);
        expect(
          find.descendant(of: transfers, matching: find.text('1')),
          findsOne,
        );
        expect(
          find.descendant(of: notifications, matching: find.text('2')),
          findsOne,
        );
      }

      expectOneGroup('detailed');
      // Capture reads as a glyph and a word; the same cell acts.
      expect(find.byIcon(LucideIcons.circleDot), findsOneWidget);
      expect(find.byIcon(LucideIcons.pause), findsOneWidget);
      await tester.tap(cells.first);
      await tester.pumpAndSettle();
      expect(find.text('Paused'), findsOneWidget);
      expect(find.byIcon(LucideIcons.circlePause), findsOneWidget);
      expect(find.byIcon(LucideIcons.play), findsOneWidget);

      // Icon-only cells stay one object.
      await pump(tester, width: 820);
      expect(stageOf(tester), HeaderStage.compact);
      expectOneGroup('compact');
      expect(find.text('Paused'), findsNothing);

      // Keyboard focus moves cell by cell, then to the end controls.
      bool focusedWithin(Finder finder) {
        final focused = FocusManager.instance.primaryFocus?.context;
        if (focused == null) return false;
        final target = tester.element(finder);
        var found = identical(focused, target);
        focused.visitAncestorElements((element) {
          found = found || identical(element, target);
          return !found;
        });
        return found;
      }

      for (var step = 0; step < 12 && !focusedWithin(cells.first); step++) {
        await tester.sendKeyEvent(LogicalKeyboardKey.tab);
        await tester.pump();
      }
      for (final target in [...cells, transfers, notifications]) {
        expect(focusedWithin(target), isTrue, reason: '$target');
        await tester.sendKeyEvent(LogicalKeyboardKey.tab);
        await tester.pump();
      }
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant.only(TargetPlatform.windows),
  );

  testWidgets(
    'storage cell states its usage in words and is left out while unknown',
    (tester) async {
      final cell = find.byKey(const ValueKey<String>('storage-header-open'));
      final capture = find.byKey(
        const ValueKey<String>('capture-header-toggle'),
      );
      final sync = find.byKey(const ValueKey<String>('sync-header-open'));
      storage.value = null;
      await pump(tester, width: 700);
      expect(stageOf(tester), HeaderStage.compact);
      expect(cell, findsNothing);
      final unknownEnd = tester.getRect(capture).right;

      storage.value = const StorageUsage(
        usedBytes: 2254857830,
        quotaBytes: 5 * _gib,
      );
      await tester.pump();
      expect(cell, findsOneWidget);
      expect(tester.getRect(cell).left, tester.getRect(sync).right);
      expect(tester.getRect(capture).right, lessThan(unknownEnd));
      expect(
        find.bySemanticsLabel('Storage: 2.1 GiB used of 5 GiB'),
        findsOneWidget,
      );
      await tester.tap(cell);
      expect(storageOpened, 1);

      // Over the quota is said in words, and by the glyph without them.
      storage.value = const StorageUsage(
        usedBytes: 5 * _gib + 300 * 1024 * 1024,
        quotaBytes: 5 * _gib,
      );
      await tester.pump();
      expect(
        find.descendant(
          of: cell,
          matching: find.byIcon(LucideIcons.circleAlert),
        ),
        findsOneWidget,
      );
      expect(
        find.bySemanticsLabel('Storage: 5.3 GiB used of 5 GiB · Over quota'),
        findsOneWidget,
      );
      await pump(tester, width: 1400);
      expect(find.text('Over quota'), findsOneWidget);
      expect(find.text(' · 5.3 of 5 GiB'), findsOneWidget);

      // Without a quota only the amount used is shown.
      storage.value = const StorageUsage(usedBytes: 2254857830, quotaBytes: 0);
      await tester.pump();
      expect(find.text('2.1 GiB'), findsOneWidget);
      expect(
        find.descendant(of: cell, matching: find.byIcon(LucideIcons.hardDrive)),
        findsOneWidget,
      );
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant.only(TargetPlatform.windows),
  );

  testWidgets('header rebuilds keep the status cells subscribed', (
    tester,
  ) async {
    final counted = _CountedUsage(storage.value);
    storage.dispose();
    storage = counted;
    await pump(tester, width: 1400);
    final added = counted.added;
    final removed = counted.removed;
    expect(added, greaterThan(removed));

    for (final destination in [
      AppDestination.devices,
      AppDestination.history,
    ]) {
      navigation.selectDestination(destination);
      await tester.pumpAndSettle();
    }
    expect(counted.added, added);
    expect(counted.removed, removed);
    expect(tester.takeException(), isNull);
  }, variant: TargetPlatformVariant.only(TargetPlatform.windows));

  testWidgets(
    'moving navigation between header and bottom bar keeps state and panels',
    (tester) async {
      final scroll = ScrollController();
      addTearDown(scroll.dispose);
      var mounts = 0;
      final history = ListView(
        controller: scroll,
        children: [
          _MountProbe(onMount: () => mounts++),
          for (var index = 0; index < 40; index++)
            Padding(
              padding: const EdgeInsets.all(AppSpacing.xxl),
              child: Text('Row $index'),
            ),
        ],
      );
      await pump(tester, width: 1100, history: history);
      expect(stageOf(tester).showsHeaderNavigation, isTrue);
      scroll.jumpTo(300);
      await tester.tap(find.byKey(const ValueKey<String>('sync-header-open')));
      await tester.pumpAndSettle();
      final panel = find.byKey(const ValueKey<String>('sync-details-drawer'));
      expect(panel, findsOneWidget);

      for (final width in [600.0, 1100.0]) {
        await pump(tester, width: width, history: history);
        await tester.pumpAndSettle();
        expect(stageOf(tester).showsHeaderNavigation, width == 1100);
        expect(panel, findsOneWidget);
        expect(mounts, 1);
        expect(scroll.offset, 300);
        expect(navigation.selectedDestination, AppDestination.history);
      }
      await tester.tap(
        find.byKey(const ValueKey<String>('close-sync-details')),
      );
      await tester.pumpAndSettle();
      expect(panel, findsNothing);
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant.only(TargetPlatform.windows),
  );
}

Widget _countedAction(String key, IconData icon, int count) => Button.secondary(
  key: ValueKey<String>(key),
  style: AppTheme.headerActionButtonStyle(const ButtonStyle.secondaryIcon()),
  onPressed: () {},
  child: HeaderActionContent(
    icon: Icon(icon),
    label: '$count',
    labelUntil: HeaderStage.bottomNavigation,
  ),
);

class _CountedUsage extends ValueNotifier<StorageUsage?> {
  _CountedUsage(super.value);

  int added = 0;
  int removed = 0;

  @override
  void addListener(VoidCallback listener) {
    added++;
    super.addListener(listener);
  }

  @override
  void removeListener(VoidCallback listener) {
    removed++;
    super.removeListener(listener);
  }
}

class _MountProbe extends StatefulWidget {
  const _MountProbe({required this.onMount});

  final VoidCallback onMount;

  @override
  State<_MountProbe> createState() => _MountProbeState();
}

class _MountProbeState extends State<_MountProbe> {
  @override
  void initState() {
    super.initState();
    widget.onMount();
  }

  @override
  Widget build(BuildContext context) => const SizedBox.shrink();
}

/// Stands in for the Devices commands with the same header content as
/// `DevicesHeaderActions`: labelled until [HeaderStage.iconCommands].
class _DeviceCommands extends StatelessWidget {
  const _DeviceCommands();

  @override
  Widget build(BuildContext context) => Row(
    mainAxisSize: MainAxisSize.min,
    spacing: AppSpacing.sm,
    children: [
      for (final (key, label) in _deviceCommands)
        Button.secondary(
          key: ValueKey<String>(key),
          style: AppTheme.headerActionButtonStyle(
            const ButtonStyle.secondaryIcon(),
          ),
          onPressed: () {},
          child: HeaderActionContent(
            icon: const Icon(LucideIcons.plus),
            label: label,
            labelUntil: HeaderStage.iconCommands,
          ),
        ),
    ],
  );
}

const _gib = 1024 * 1024 * 1024;
