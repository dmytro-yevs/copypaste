import 'dart:ui' show SemanticsRole, Tristate;

import 'package:copypaste_flutter/app/app.dart';
import 'package:copypaste_flutter/app/navigation/navigation.dart';
import 'package:copypaste_flutter/app/shell/shell.dart';
import 'package:copypaste_flutter/app/theme/app_overlays.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/app/theme/app_tokens.dart';
import 'package:copypaste_flutter/features/devices/device_presentation.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';
import 'package:window_manager/window_manager.dart';

import '../app_features_test_support.dart';

void main() {
  for (final platform in [
    TargetPlatform.macOS,
    TargetPlatform.windows,
    TargetPlatform.linux,
  ]) {
    testWidgets(
      'desktop app commands and system-key passthrough on $platform',
      (tester) async {
        final navigation = AppNavigationController(
          initialDestination: AppDestination.settings,
        );
        addTearDown(navigation.dispose);
        var closes = 0;
        var quits = 0;
        var toggles = 0;
        var exits = 0;
        var refreshes = 0;
        await tester.pumpWidget(
          _shell(
            controller: navigation,
            onCloseWindow: () async {
              closes += 1;
            },
            onQuit: () async {
              quits += 1;
            },
            onToggleFullScreen: () async {
              toggles += 1;
            },
            onExitFullScreen: () async {
              exits += 1;
            },
            onRefresh: () {
              refreshes += 1;
            },
          ),
        );
        await tester.pumpAndSettle();
        final modifier = platform == TargetPlatform.macOS
            ? LogicalKeyboardKey.metaLeft
            : LogicalKeyboardKey.controlLeft;
        await tester.sendKeyDownEvent(modifier);
        for (final key in [
          LogicalKeyboardKey.keyF,
          LogicalKeyboardKey.keyR,
          LogicalKeyboardKey.keyW,
          LogicalKeyboardKey.keyQ,
        ]) {
          await tester.sendKeyEvent(key);
        }
        await tester.sendKeyUpEvent(modifier);
        await tester.pumpAndSettle();
        expect(navigation.selectedDestination, AppDestination.history);
        expect(navigation.historyFindRequests.value, isTrue);
        expect([closes, quits, refreshes], [1, 1, 1]);
        await tester.sendKeyEvent(LogicalKeyboardKey.f11);
        await tester.pumpAndSettle();
        expect(toggles, platform == TargetPlatform.macOS ? 0 : 1);
        await tester.sendKeyEvent(LogicalKeyboardKey.escape);
        await tester.pumpAndSettle();
        expect(exits, 1);
        // Window switching, tiling, and force-quit remain owned by the OS.
        await tester.sendKeyDownEvent(LogicalKeyboardKey.altLeft);
        await tester.sendKeyEvent(LogicalKeyboardKey.tab);
        await tester.sendKeyUpEvent(LogicalKeyboardKey.altLeft);
        await tester.sendKeyDownEvent(LogicalKeyboardKey.metaLeft);
        await tester.sendKeyEvent(LogicalKeyboardKey.arrowLeft);
        await tester.sendKeyUpEvent(LogicalKeyboardKey.metaLeft);
        await tester.pumpAndSettle();
        expect(
          [closes, quits, toggles, refreshes],
          [1, 1, platform == TargetPlatform.macOS ? 0 : 1, 1],
        );
      },
      variant: TargetPlatformVariant({platform}),
    );
  }
  for (final platform in [
    TargetPlatform.macOS,
    TargetPlatform.windows,
    TargetPlatform.android,
  ]) {
    for (final width in [390.0, 1400.0]) {
      testWidgets(
        'platform Settings shortcut and header return to History at $width',
        (tester) async {
          await tester.binding.setSurfaceSize(Size(width, 900));
          addTearDown(() => tester.binding.setSurfaceSize(null));
          final controller = AppNavigationController();
          addTearDown(controller.dispose);
          await tester.pumpWidget(_shell(controller: controller));
          await tester.pump(const Duration(milliseconds: 500));
          final modifier = platform == TargetPlatform.macOS
              ? LogicalKeyboardKey.metaLeft
              : LogicalKeyboardKey.controlLeft;
          await tester.sendKeyDownEvent(modifier);
          await tester.sendKeyEvent(LogicalKeyboardKey.comma);
          await tester.sendKeyUpEvent(modifier);
          await tester.pump(const Duration(milliseconds: 500));
          expect(controller.selectedDestination, AppDestination.settings);
          await tester.tap(
            find.byKey(
              ValueKey(
                width < 640
                    ? 'header-brand-logo'
                    : 'header-destination-history',
              ),
            ),
          );
          await tester.pump(const Duration(milliseconds: 500));
          expect(controller.selectedDestination, AppDestination.history);
          expect(tester.takeException(), isNull);
        },
        variant: TargetPlatformVariant({platform}),
      );
    }
  }

  testWidgets('mobile header logo keeps the shared header height', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(390, 844));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(_shell());
    final header = find.byType(AppBar);
    expect(
      tester.getSize(header).height,
      AppTheme.headerHeight(tester.element(header)),
    );
    final logo = find.byKey(const ValueKey<String>('header-brand-logo'));
    final title = find.descendant(
      of: find.byType(AppBar),
      matching: find.text('History'),
    );
    expect(logo, findsOneWidget);
    expect(tester.getRect(logo).right, lessThan(tester.getRect(title).left));
  });
  testWidgets(
    'Settings header centers its title, logo, and action icon',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(390, 844));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(
        _shell(
          headerActions: {
            AppDestination.settings: [
              Button.secondary(
                style: const ButtonStyle.secondaryIcon(),
                onPressed: () {},
                child: const Icon(LucideIcons.network),
              ),
            ],
          },
        ),
      );
      await tester.tap(_bottomNavigationItem('Settings'));
      await tester.pumpAndSettle();
      final header = find.byType(AppBar);
      final title = find.descendant(
        of: header,
        matching: find.text('Settings'),
      );
      final logo = find.byKey(const ValueKey('header-brand-logo'));
      final action = find.descendant(
        of: header,
        matching: find.byIcon(LucideIcons.network),
      );
      expect(
        tester.getCenter(title).dy,
        closeTo(tester.getCenter(logo).dy, 0.01),
      );
      expect(
        tester.getCenter(title).dy,
        closeTo(tester.getCenter(action).dy, 0.01),
      );
      expect(
        DefaultTextStyle.of(tester.element(title)).style.leadingDistribution,
        TextLeadingDistribution.even,
      );
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant({
      TargetPlatform.android,
      TargetPlatform.macOS,
      TargetPlatform.windows,
      TargetPlatform.linux,
    }),
  );

  test('includes the recovered CopyPaste brand asset', () async {
    final asset = await rootBundle.load('assets/brand/copypaste.png');

    expect(asset.lengthInBytes, greaterThan(0));
  });

  testWidgets('uses the shadcn navigation bar below the compact breakpoint', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(639, 720));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    await tester.pumpWidget(_shell());

    expect(find.byType(NavigationBar), findsOneWidget);
    expect(_bottomNavigationItem('History'), findsOneWidget);
    expect(_bottomNavigationItem('Devices'), findsOneWidget);
    expect(_bottomNavigationItem('Settings'), findsOneWidget);
    await tester.tap(_bottomNavigationItem('Devices'));
    await tester.pumpAndSettle();
    expect(find.text('Devices body'), findsOneWidget);
  });

  testWidgets('renders a compact capsule with every label below its icon', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(639, 720));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    await tester.pumpWidget(_shell());

    final navigationBar = tester.widget<NavigationBar>(
      find.byKey(const ValueKey<String>('bottom-navigation')),
    );
    final bounds = tester.getRect(
      find.byKey(const ValueKey<String>('bottom-navigation')),
    );
    final dock = find.byKey(const ValueKey<String>('mobile-navigation-dock'));
    final dockBounds = tester.getRect(dock);

    expect(
      navigationBar.selectedKey,
      const ValueKey<AppDestination>(AppDestination.history),
    );
    expect(navigationBar.labelType, NavigationLabelType.all);
    expect(navigationBar.labelPosition, NavigationLabelPosition.bottom);
    expect(navigationBar.alignment, NavigationBarAlignment.center);
    expect(navigationBar.backgroundColor, Colors.transparent);
    expect(navigationBar.padding, AppTheme.mobileNavigationPadding);
    expect(navigationBar.spacing, AppSpacing.zero);
    final surfaceBounds = tester.getRect(
      find.byKey(const ValueKey('mobile-navigation-surface')),
    );
    expect(surfaceBounds.width, AppLayoutSize.mobileNavigationMaxWidth);
    expect(surfaceBounds.height, AppControlSize.navigation);
    expect(dockBounds.left, 0);
    expect(dockBounds.width, 639);
    expect(bounds.width, lessThan(639 - AppSpacing.lg * 2));
    expect(bounds.center.dx, closeTo(dockBounds.center.dx, 0.5));
    expect(
      find.descendant(of: dock, matching: find.byType(Card)),
      findsNothing,
    );
    final items = tester.widgetList<NavigationItem>(
      find.descendant(
        of: find.byType(NavigationBar),
        matching: find.byType(NavigationItem),
      ),
    );
    expect(items, hasLength(3));
    expect(
      items.map(
        (item) => ((item.label! as ExcludeSemantics).child as Text).data,
      ),
      ['History', 'Devices', 'Settings'],
    );
    expect(
      tester
          .widgetList<Icon>(
            find.descendant(
              of: find.byType(NavigationBar),
              matching: find.byType(Icon),
            ),
          )
          .map((item) => item.icon),
      [
        LucideIcons.clipboardList,
        DevicePresentation.collectionIcon,
        LucideIcons.settings,
      ],
    );
    final selectedItem = find.descendant(
      of: dock,
      matching: find.byKey(
        const ValueKey<AppDestination>(AppDestination.history),
      ),
    );
    final selectedDecoration = tester
        .widget<DecoratedBox>(
          find.byKey(const ValueKey('mobile-navigation-selection-0')),
        )
        .decoration;
    expect(
      (selectedDecoration as BoxDecoration).color,
      AppTheme.navigationAccent.withValues(
        alpha: AppTheme.mobileNavigationSelectionOpacity,
      ),
    );
    expect(
      items.first.selectedStyle!.iconTheme(tester.element(selectedItem), const {
        WidgetState.selected,
      }).color,
      AppTheme.navigationAccent,
    );

    for (final destination in appNavigationDestinations) {
      await tester.tap(_bottomNavigationItem(destination.label));
      await tester.pumpAndSettle();
      expect(find.text('${destination.label} body'), findsOneWidget);
      expect(
        tester.getRect(find.byType(NavigationBar)).width,
        closeTo(bounds.width, 0.5),
      );
      for (final item in appNavigationDestinations) {
        final target = _bottomNavigationItem(item.label);
        final label = find.descendant(
          of: target,
          matching: find.text(item.label),
        );
        final icon = find.descendant(of: target, matching: find.byType(Icon));
        expect(label.hitTestable(), findsOneWidget);
        expect(
          tester.renderObject<RenderParagraph>(label).text.style!.fontSize,
          12,
        );
        expect(
          tester.getRect(label).top,
          greaterThan(tester.getRect(icon).bottom),
        );
        expect(
          tester.getCenter(label).dx,
          closeTo(tester.getCenter(icon).dx, 0.5),
        );
      }
    }
  });

  testWidgets(
    'mobile dock floats over content and keeps its margins interactive',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(390, 720));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      var contentTaps = 0;
      final controller = AppNavigationController();
      addTearDown(controller.dispose);
      await tester.pumpWidget(
        _shell(
          controller: controller,
          destinations: {
            AppDestination.history: Listener(
              onPointerDown: (_) => contentTaps++,
              child: ColoredBox(
                key: const ValueKey('floating-content'),
                color: Colors.white,
                child: ListView(
                  children: [
                    for (var index = 0; index < 30; index++)
                      Padding(
                        padding: const EdgeInsets.all(AppSpacing.xxl),
                        child: Text('Row $index'),
                      ),
                  ],
                ),
              ),
            ),
            AppDestination.devices: const Text('Devices body'),
            AppDestination.settings: const Text('Settings body'),
          },
        ),
      );
      final dockBounds = tester.getRect(
        find.byKey(const ValueKey('mobile-navigation-dock')),
      );
      expect(
        tester.getRect(find.byKey(const ValueKey('floating-content'))).bottom,
        720,
      );
      await tester.tapAt(Offset(AppSpacing.xs, dockBounds.center.dy));
      expect(contentTaps, 1);
      await tester.drag(find.byType(ListView), const Offset(0, -5000));
      await tester.pumpAndSettle();
      expect(
        tester.getRect(find.text('Row 29')).bottom,
        lessThanOrEqualTo(dockBounds.top),
      );
      await tester.tap(_bottomNavigationItem('Devices'));
      await tester.pumpAndSettle();
      expect(controller.selectedDestination, AppDestination.devices);
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets('hides mobile navigation while a bottom overlay is open', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(480, 720));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final controller = AppNavigationController();
    addTearDown(controller.dispose);

    await tester.pumpWidget(_shell(controller: controller));
    expect(
      find.byKey(const ValueKey<String>('bottom-navigation')),
      findsOneWidget,
    );

    controller.setBottomOverlayOpen(true);
    await tester.pump();

    expect(
      find.byKey(const ValueKey<String>('bottom-navigation')),
      findsNothing,
    );

    controller.setBottomOverlayOpen(false);
    await tester.pump();

    expect(
      find.byKey(const ValueKey<String>('bottom-navigation')),
      findsOneWidget,
    );
  });

  testWidgets('compact editors hide navigation until their drawer closes', (
    tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(400, 700);
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final controller = AppNavigationController();
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      _shell(
        controller: controller,
        destinations: {
          AppDestination.history: Builder(
            builder: (context) => Button.primary(
              child: const Text('Open editor'),
              onPressed: () => AppOverlays.showCompactEditor<void>(
                context,
                builder: (context) => Button.outline(
                  onPressed: () => closeOverlay(context),
                  child: const Text('Close editor'),
                ),
              ),
            ),
          ),
          AppDestination.devices: const Text('Devices body'),
          AppDestination.settings: const Text('Settings body'),
        },
      ),
    );
    final navigation = find.byKey(const ValueKey<String>('bottom-navigation'));
    expect(navigation, findsOneWidget);
    await tester.tap(find.text('Open editor'));
    await tester.pumpAndSettle();
    expect(navigation, findsNothing);
    await tester.tap(find.text('Close editor'));
    await tester.pumpAndSettle();
    expect(navigation, findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  for (final mode in [ThemeMode.light, ThemeMode.dark]) {
    testWidgets(
      'capsule keeps every tab accessible at 320px with 200% text in $mode',
      (tester) async {
        await tester.binding.setSurfaceSize(const Size(320, 600));
        addTearDown(() => tester.binding.setSurfaceSize(null));
        final semantics = tester.ensureSemantics();
        final controller = AppNavigationController();
        addTearDown(controller.dispose);

        try {
          await tester.pumpWidget(
            ShadcnApp(
              theme: AppTheme.light,
              darkTheme: AppTheme.dark,
              themeMode: mode,
              builder: AppTheme.builder,
              home: MediaQuery(
                data: const MediaQueryData(
                  textScaler: TextScaler.linear(2),
                  padding: EdgeInsets.only(bottom: 24),
                ),
                child: AppShell(
                  controller: controller,
                  destinations: const {
                    AppDestination.history: Text('History body'),
                    AppDestination.devices: Text('Devices body'),
                    AppDestination.settings: Text('Settings body'),
                  },
                ),
              ),
            ),
          );

          for (final destination in appNavigationDestinations) {
            await tester.tap(_bottomNavigationItem(destination.label));
            await tester.pumpAndSettle();
            expect(controller.selectedDestination, destination.destination);
            expect(find.text('${destination.label} body'), findsOneWidget);
            final bar = find.byKey(const ValueKey<String>('bottom-navigation'));
            final barBounds = tester.getRect(bar);
            expect(barBounds.left, greaterThanOrEqualTo(AppSpacing.sm));
            expect(barBounds.right, lessThanOrEqualTo(320 - AppSpacing.sm));
            expect(barBounds.center.dx, closeTo(160, 0.5));
            expect(
              barBounds.bottom,
              lessThanOrEqualTo(600 - 24 - AppSpacing.sm),
            );

            for (final item in appNavigationDestinations) {
              final target = _bottomNavigationItem(item.label);
              final size = tester.getSize(target);
              expect(size.width, greaterThanOrEqualTo(AppControlSize.touch));
              expect(size.height, greaterThanOrEqualTo(AppControlSize.touch));
              expect(
                find.descendant(
                  of: bar,
                  matching: find.bySemanticsLabel(item.label),
                ),
                findsOneWidget,
              );
              final label = find.descendant(
                of: target,
                matching: find.text(item.label),
              );
              expect(label.hitTestable(), findsOneWidget);
              final targetBounds = tester.getRect(target);
              expect(targetBounds.left, greaterThanOrEqualTo(barBounds.left));
              expect(targetBounds.right, lessThanOrEqualTo(barBounds.right));
              final iconBounds = tester.getRect(
                find.descendant(of: target, matching: find.byType(Icon)),
              );
              final labelBounds = tester.getRect(label);
              expect(labelBounds.top, greaterThan(iconBounds.bottom));
              expect(
                labelBounds.center.dx,
                closeTo(targetBounds.center.dx, 0.5),
              );
              expect(
                iconBounds.center.dx,
                closeTo(targetBounds.center.dx, 0.5),
              );
              expect(
                tester
                    .renderObject<RenderParagraph>(label)
                    .text
                    .style!
                    .fontSize,
                12,
              );
              expect(
                MediaQuery.textScalerOf(tester.element(label)).scale(12),
                24,
              );
            }
            expect(tester.takeException(), isNull);
          }
        } finally {
          semantics.dispose();
        }
      },
      variant: const TargetPlatformVariant({
        TargetPlatform.android,
        TargetPlatform.macOS,
        TargetPlatform.windows,
        TargetPlatform.linux,
      }),
    );
  }

  testWidgets(
    'hides capsule for the keyboard and restores it above safe area',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(390, 600));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      addTearDown(tester.view.resetViewInsets);
      final controller = AppNavigationController();
      addTearDown(controller.dispose);
      double? contentBottomPadding;
      await tester.pumpWidget(
        _shell(
          controller: controller,
          destinations: {
            AppDestination.history: Builder(
              builder: (context) {
                contentBottomPadding = MediaQuery.paddingOf(context).bottom;
                return const Text('History body');
              },
            ),
            AppDestination.devices: const SizedBox.expand(),
            AppDestination.settings: const SizedBox.expand(),
          },
        ),
      );
      final bar = find.byKey(const ValueKey<String>('bottom-navigation'));
      expect(bar, findsOneWidget);
      final footerHeight = tester
          .getSize(find.byKey(const ValueKey('mobile-navigation-dock')))
          .height;
      expect(contentBottomPadding, closeTo(footerHeight, 0.01));
      tester.view.viewInsets = const FakeViewPadding(bottom: 200);
      await tester.pump();
      expect(bar, findsNothing);
      expect(contentBottomPadding, 0);
      tester.view.resetViewInsets();
      await tester.pump();
      expect(bar, findsOneWidget);
      expect(contentBottomPadding, closeTo(footerHeight, 0.01));
      controller.setBottomOverlayOpen(true);
      await tester.pump();
      expect(contentBottomPadding, 0);
      controller.setBottomOverlayOpen(false);
      await tester.pump();
      expect(contentBottomPadding, closeTo(footerHeight, 0.01));
      await tester.binding.setSurfaceSize(const Size(640, 600));
      await tester.pump();
      expect(contentBottomPadding, 0);
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant({
      TargetPlatform.android,
      TargetPlatform.macOS,
      TargetPlatform.windows,
      TargetPlatform.linux,
    }),
  );

  testWidgets('carries the destinations in the header from the breakpoint', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(640, 720));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final controller = AppNavigationController();
    addTearDown(controller.dispose);

    await tester.pumpWidget(_shell(controller: controller));

    expect(find.byType(NavigationRail), findsNothing);
    expect(find.byType(NavigationBar), findsNothing);
    final header = find.byType(AppHeader);
    final items = [
      for (final destination in AppDestination.values)
        find.byKey(ValueKey('header-destination-${destination.name}')),
    ];
    for (final (index, item) in items.indexed) {
      expect(find.descendant(of: header, matching: item), findsOneWidget);
      expect(
        find.descendant(
          of: item,
          matching: find.text(appNavigationDestinations[index].label),
        ),
        findsOneWidget,
      );
      if (index > 0) {
        expect(
          tester.getRect(item).left,
          greaterThanOrEqualTo(tester.getRect(items[index - 1]).right),
        );
      }
    }
    // The destination title is not repeated beside the navigation.
    expect(
      find.descendant(of: header, matching: find.text('History')),
      findsOne,
    );
    expect(tester.getSize(find.text('History body')).width, 640);

    // The current destination recolours the header border under its item.
    final headerRect = tester.getRect(header);
    Finder indicator(Finder item) => find.ancestor(
      of: item,
      matching: find.byWidgetPredicate(
        (widget) =>
            widget is DecoratedBox &&
            widget.decoration == AppTheme.headerNavigationSelectionDecoration,
      ),
    );
    expect(tester.getRect(indicator(items[0])).bottom, headerRect.bottom);
    expect(
      tester.getRect(indicator(items[0])).width,
      tester.getRect(items[0]).width,
    );
    expect(indicator(items[1]), findsNothing);

    // Pointer hover and keyboard focus are both visible on an item.
    final itemContext = tester.element(items[1]);
    final colors = Theme.of(itemContext).colorScheme;
    final style = tester.widget<Button>(items[1]).style;
    BoxDecoration decoration(WidgetState state) =>
        style.decoration(itemContext, {state}) as BoxDecoration;
    expect(decoration(WidgetState.hovered).color, colors.border);
    expect(decoration(WidgetState.focused).border!.top.color, colors.primary);
    expect(
      style.textStyle(itemContext, const {}).color,
      colors.mutedForeground,
    );
    expect(
      tester.widget<Button>(items[0]).style.textStyle(itemContext, const {}),
      isA<TextStyle>()
          .having((text) => text.color, 'color', colors.foreground)
          .having(
            (text) => text.fontWeight,
            'weight',
            AppTheme.headerNavigationSelectedWeight,
          ),
    );

    await tester.tap(items[1]);
    await tester.pump();
    expect(find.text('Devices body'), findsOneWidget);
    expect(indicator(items[0]), findsNothing);
    expect(indicator(items[1]), findsOneWidget);

    // Activating the current destination again returns it to its root.
    final detail = controller.push<void>(
      tester.element(find.text('Devices body')),
      builder: (context) => const Scaffold(child: Text('Detail route')),
    );
    await tester.pumpAndSettle();
    expect(find.text('Detail route'), findsOneWidget);
    await tester.tap(items[1]);
    await tester.pumpAndSettle();
    await detail;
    expect(find.text('Detail route'), findsNothing);
    expect(controller.selectedDestination, AppDestination.devices);
    expect(tester.takeException(), isNull);
  });

  for (final layout in [
    (name: 'unified desktop', size: const Size(1200, 720), unified: true),
    (name: 'desktop', size: const Size(1200, 720), unified: false),
    (name: 'mobile', size: const Size(390, 844), unified: false),
  ]) {
    testWidgets('drawers preserve header actions on ${layout.name}', (
      tester,
    ) async {
      await tester.binding.setSurfaceSize(layout.size);
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final controller = AppNavigationController();
      addTearDown(controller.dispose);
      var presses = 0;
      await tester.pumpWidget(
        _shell(
          controller: controller,
          unifiedTitleBar: layout.unified,
          headerActions: {
            AppDestination.history: [
              Builder(
                builder: (context) => Button.secondary(
                  key: const ValueKey('open-test-drawer'),
                  style: AppTheme.controlButtonStyle(
                    const ButtonStyle.secondaryIcon(),
                  ),
                  onPressed: () => showOverlay<void>(
                    context,
                    AppOverlays.drawerConfiguration(context),
                    builder: (context) => ConstrainedBox(
                      constraints: AppOverlays.drawerContentConstraints(
                        context,
                      ),
                      child: Scaffold(
                        child: Button.outline(
                          onPressed: () => closeDrawer(context),
                          child: const Text('Close test drawer'),
                        ),
                      ),
                    ),
                  ),
                  child: const Icon(LucideIcons.bell),
                ),
              ),
              Button.secondary(
                key: const ValueKey('test-header-action'),
                style: AppTheme.controlButtonStyle(
                  const ButtonStyle.secondaryIcon(),
                ),
                onPressed: () => presses++,
                child: const Icon(LucideIcons.refreshCw),
              ),
              if (layout.name == 'mobile') ...[
                for (final icon in [LucideIcons.download, LucideIcons.pause])
                  Button.secondary(
                    style: AppTheme.controlButtonStyle(
                      const ButtonStyle.secondaryIcon(),
                    ),
                    onPressed: () {},
                    child: Icon(icon),
                  ),
              ],
            ],
          },
        ),
      );
      // Expanded navigation uses a continuously ticking library marquee.
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 500));
      await tester.pump();
      final action = find.byKey(const ValueKey('test-header-action'));
      final headerRect = tester.getRect(action);
      await tester.tap(find.byKey(const ValueKey('open-test-drawer')));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 500));
      await tester.pump();
      expect(find.text('Close test drawer'), findsOneWidget);
      expect(
        tester
            .getRect(find.byKey(const ValueKey('app-body-drawer-overlay')))
            .top,
        greaterThan(headerRect.bottom),
      );
      expect(tester.getRect(action), headerRect);
      expect(action.hitTestable(), findsOneWidget);
      await tester.tap(action);
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 500));
      await tester.pump();
      expect(presses, 1);
      expect(find.text('Close test drawer'), findsOneWidget);
      await tester.tap(find.text('Close test drawer'));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 500));
      await tester.pump();
      expect(find.text('Close test drawer'), findsNothing);
      expect(tester.takeException(), isNull);
    });
  }

  testWidgets('shows only the selected destination header actions', (
    tester,
  ) async {
    final controller = AppNavigationController();
    addTearDown(controller.dispose);

    await tester.pumpWidget(
      _shell(
        controller: controller,
        headerActions: const {
          AppDestination.devices: [Text('Device header actions')],
        },
      ),
    );

    expect(find.text('Device header actions'), findsNothing);

    controller.selectDestination(AppDestination.devices);
    await tester.pump();

    expect(
      find.ancestor(
        of: find.text('Device header actions'),
        matching: find.byType(AppBar),
      ),
      findsOneWidget,
    );

    controller.selectDestination(AppDestination.settings);
    await tester.pump();

    expect(find.text('Device header actions'), findsNothing);
  });

  testWidgets(
    'places navigation in the unified macOS header after the native control reserve',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(1200, 720));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final controller = AppNavigationController();
      addTearDown(controller.dispose);

      await tester.pumpWidget(
        _shell(
          controller: controller,
          unifiedTitleBar: true,
          headerActions: {
            AppDestination.devices: [
              Button.secondary(
                key: const ValueKey('device-command'),
                onPressed: () {},
                child: const Text('Device command'),
              ),
            ],
          },
        ),
      );

      final header = find.byKey(const ValueKey<String>('macos-window-header'));
      final reserve = find.byKey(
        const ValueKey<String>('macos-native-controls-reserve'),
      );
      final drag = find.byKey(
        const ValueKey<String>('macos-window-drag-region'),
      );
      expect(header, findsOneWidget);
      expect(find.byType(DragToMoveArea), findsOneWidget);
      expect(tester.getRect(reserve).left, 0);
      expect(
        tester.getSize(reserve).width,
        MacosWindowHeader.nativeControlsWidth,
      );
      expect(tester.getSize(header), const Size(1200, AppControlSize.touch));
      expect(
        tester.widget<AppBar>(header).backgroundColor,
        Theme.of(tester.element(header)).colorScheme.muted,
      );
      expect(
        tester.getRect(header).bottom,
        tester.getRect(find.text('History body')).top,
      );

      // Nothing sits under the window buttons, and only free space drags.
      final wordmark = find.byKey(const ValueKey<String>('header-wordmark'));
      expect(
        tester.getRect(wordmark).left,
        greaterThanOrEqualTo(tester.getRect(reserve).right),
      );
      controller.selectDestination(AppDestination.devices);
      await tester.pump();
      final dragRect = tester.getRect(drag);
      expect(dragRect.width, greaterThan(0));
      for (final control in [
        for (final destination in AppDestination.values)
          find.byKey(ValueKey('header-destination-${destination.name}')),
        find.byKey(const ValueKey('device-command')),
      ]) {
        expect(find.descendant(of: drag, matching: control), findsNothing);
        expect(tester.getRect(control).overlaps(dragRect), isFalse);
      }
      expect(
        find.descendant(of: header, matching: find.text('Devices')),
        findsOne,
      );
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant.only(TargetPlatform.macOS),
  );

  testWidgets('keeps the standard content header by default', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1200, 720));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    await tester.pumpWidget(_shell());

    expect(
      find.byKey(const ValueKey<String>('macos-native-controls-reserve')),
      findsNothing,
    );
    expect(find.byType(DragToMoveArea), findsNothing);
    final header = find.byType(AppBar);
    expect(tester.getRect(header).width, 1200);
    expect(
      find.descendant(
        of: header,
        matching: find.byKey(const ValueKey('header-destination-history')),
      ),
      findsOneWidget,
    );
  });

  testWidgets(
    'keeps library navigation usable with scaled text at 320 pixels',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(320, 480));
      addTearDown(() => tester.binding.setSurfaceSize(null));

      await tester.pumpWidget(
        ShadcnApp(
          theme: AppTheme.light,
          darkTheme: AppTheme.dark,
          themeMode: AppTheme.mode,
          builder: AppTheme.builder,
          home: MediaQuery(
            data: const MediaQueryData(textScaler: TextScaler.linear(2)),
            child: _testShell(),
          ),
        ),
      );

      expect(
        ((tester
                            .widgetList<NavigationItem>(
                              find.descendant(
                                of: find.byType(NavigationBar),
                                matching: find.byType(NavigationItem),
                              ),
                            )
                            .last
                            .label!
                        as ExcludeSemantics)
                    .child
                as Text)
            .data,
        'Settings',
      );
      expect(_bottomNavigationItem('History'), findsOneWidget);
      expect(_bottomNavigationItem('Devices'), findsOneWidget);
      expect(_bottomNavigationItem('Settings'), findsOneWidget);
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets('retains form, scroll, and a detail route through resize', (
    tester,
  ) async {
    final controller = AppNavigationController();
    final initializations = ValueNotifier<int>(0);
    final inputController = TextEditingController();
    final scrollController = ScrollController();
    addTearDown(controller.dispose);
    addTearDown(initializations.dispose);
    addTearDown(inputController.dispose);
    addTearDown(scrollController.dispose);
    await tester.binding.setSurfaceSize(const Size(1200, 720));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    await tester.pumpWidget(
      _shell(
        controller: controller,
        destinations: {
          AppDestination.history: _RetainedForm(
            initializations: initializations,
            inputController: inputController,
            scrollController: scrollController,
          ),
          AppDestination.devices: const Text('Devices body'),
          AppDestination.settings: const Text('Settings body'),
        },
      ),
    );

    expect(initializations.value, 1);
    await tester.enterText(find.byType(EditableText), 'Retained text');
    await tester.drag(
      find.byKey(const ValueKey<String>('retained-form-list')),
      const Offset(0, -500),
    );
    await tester.pump();
    expect(scrollController.offset, greaterThan(0));
    final detailRoute = controller.push<void>(
      tester.element(find.text('Item 8')),
      builder: (context) => const Scaffold(child: Text('Detail route')),
    );
    await tester.pump();
    expect(find.text('Detail route'), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    expect(find.text('Detail route'), findsNothing);
    await detailRoute;
    await tester.binding.setSurfaceSize(const Size(320, 720));
    await tester.pump();
    expect(find.byType(NavigationBar), findsOneWidget);
    await tester.binding.setSurfaceSize(const Size(1200, 720));
    await tester.pump();

    expect(find.byType(NavigationBar), findsNothing);
    expect(initializations.value, 1);
    expect(inputController.text, 'Retained text');
    expect(scrollController.offset, greaterThan(0));
  });

  testWidgets('uses shadcn scaffold IME behavior in short landscape', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(640, 400));
    addTearDown(tester.view.resetViewInsets);
    addTearDown(() => tester.binding.setSurfaceSize(null));

    await tester.pumpWidget(
      _shell(
        destinations: {
          AppDestination.history: Scaffold(
            resizeToAvoidBottomInset: true,
            headers: const [
              AppBar(title: Text('History')),
              Divider(),
            ],
            footers: [
              const Divider(),
              Padding(
                padding: const EdgeInsets.all(AppSpacing.sm),
                child: Button.primary(
                  onPressed: () {},
                  child: const Text('Save'),
                ),
              ),
            ],
            child: const TextField(placeholder: Text('Search history')),
          ),
          AppDestination.devices: const SizedBox.expand(),
          AppDestination.settings: const SizedBox.expand(),
        },
      ),
    );

    expect(find.text('Save'), findsOneWidget);
    tester.view.viewInsets = const FakeViewPadding(bottom: 100);
    await tester.pump();

    expect(find.byType(Scaffold), findsNWidgets(2));
    expect(find.text('Save'), findsOneWidget);
    expect(find.byType(TextField), findsOneWidget);
    await tester.enterText(find.byType(EditableText), 'Search');
    expect(
      find.byKey(const ValueKey('header-destination-history')),
      findsOneWidget,
    );
    tester.view.resetViewInsets();
    await tester.pump();
    expect(find.text('Save'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  for (final width in [800.0, 1024.0, 480.0]) {
    testWidgets(
      'navigation exposes stable semantics and semantic tap at $width pixels',
      (tester) async {
        await tester.binding.setSurfaceSize(Size(width, 720));
        addTearDown(() => tester.binding.setSurfaceSize(null));
        final semantics = tester.ensureSemantics();
        final controller = AppNavigationController();
        addTearDown(controller.dispose);

        try {
          await tester.pumpWidget(_shell(controller: controller));
          await tester.pump(const Duration(seconds: 1));
          for (final destination in appNavigationDestinations) {
            final finder = find.semantics.byPredicate(
              (node) =>
                  node.label == destination.label &&
                  node.getSemanticsData().flagsCollection.isButton &&
                  node.getSemanticsData().hasAction(SemanticsAction.tap),
            );
            expect(finder, findsOneWidget);
            final node = finder.evaluate().single;
            expect(node.getSemanticsData().flagsCollection.isButton, isTrue);
            expect(
              node.getSemanticsData().hasAction(SemanticsAction.tap),
              isTrue,
            );
            expect(
              node.getSemanticsData().hasAction(SemanticsAction.focus),
              isTrue,
            );
            expect(
              node.getSemanticsData().flagsCollection.isSelected,
              destination.destination == AppDestination.history
                  ? Tristate.isTrue
                  : Tristate.isFalse,
            );
          }
          if (width >= 800) {
            final landmark = find.semantics.byPredicate(
              (node) =>
                  node.label == 'Main navigation' &&
                  node.getSemanticsData().role == SemanticsRole.navigation,
            );
            expect(landmark, findsOneWidget);
            expect(
              find.semantics.descendant(
                of: landmark,
                matching: find.semantics.byPredicate(
                  (node) => node.getSemanticsData().flagsCollection.isButton,
                ),
              ),
              findsNWidgets(3),
            );
            expect(find.semantics.byLabel('History body'), findsOneWidget);
          }

          final devices = find.semantics.byPredicate(
            (node) =>
                node.label == 'Devices' &&
                node.getSemanticsData().flagsCollection.isButton &&
                node.getSemanticsData().hasAction(SemanticsAction.tap),
          );
          tester.semantics.tap(devices);
          await tester.pump(const Duration(seconds: 1));

          expect(controller.selectedDestination, AppDestination.devices);
          expect(
            devices
                .evaluate()
                .single
                .getSemanticsData()
                .flagsCollection
                .isSelected,
            Tristate.isTrue,
          );
          expect(
            find.semantics
                .byPredicate(
                  (node) =>
                      node.label == 'History' &&
                      node.getSemanticsData().flagsCollection.isButton &&
                      node.getSemanticsData().hasAction(SemanticsAction.tap),
                )
                .evaluate()
                .single
                .getSemanticsData()
                .flagsCollection
                .isSelected,
            Tristate.isFalse,
          );
          if (width >= 800) {
            expect(find.semantics.byLabel('Devices body'), findsOneWidget);
          }
        } finally {
          semantics.dispose();
        }
      },
    );
  }

  testWidgets('root dialogs block background navigation semantics', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(800, 720));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final semantics = tester.ensureSemantics();
    final controller = AppNavigationController();
    addTearDown(controller.dispose);

    try {
      await tester.pumpWidget(_shell(controller: controller));
      await tester.pump(const Duration(seconds: 1));
      final historyNavigation = find.semantics.byPredicate(
        (node) =>
            node.label == 'History' &&
            node.getSemanticsData().flagsCollection.isButton &&
            node.getSemanticsData().hasAction(SemanticsAction.tap),
      );
      expect(historyNavigation, findsOneWidget);

      final dialog = AppOverlays.showDialog<void>(
        tester.element(find.byType(AppShell)),
        builder: (context) => AppOverlays.alertDialog(
          icon: LucideIcons.info,
          title: const Text('Modal navigation test'),
          actions: [
            Button.primary(
              onPressed: () => Navigator.pop(context),
              child: const Text('Close modal'),
            ),
          ],
        ),
      );
      await tester.pump(const Duration(seconds: 1));

      expect(find.text('Modal navigation test'), findsOneWidget);
      expect(historyNavigation, findsNothing);

      await tester.tap(find.text('Close modal'));
      await tester.pump(const Duration(seconds: 1));
      await dialog;

      expect(historyNavigation, findsOneWidget);
    } finally {
      semantics.dispose();
    }
  });

  testWidgets('the paste queue header action is on every destination', (
    tester,
  ) async {
    final navigation = AppNavigationController();
    addTearDown(navigation.dispose);
    final features = testAppFeatures();
    addTearDown(features.dispose);
    await features.pasteQueue.start();

    await tester.pumpWidget(
      CopyPasteApp(navigation: navigation, features: features),
    );
    for (final destination in AppDestination.values) {
      navigation.selectDestination(destination);
      await tester.pump();
      expect(
        find.byKey(const ValueKey('paste-queue-open')),
        findsOneWidget,
        reason: destination.name,
      );
    }
  });
}

Finder _bottomNavigationItem(String label) {
  return find.descendant(
    of: find.byKey(const ValueKey<String>('bottom-navigation')),
    matching: find.byKey(
      ValueKey<AppDestination>(
        appNavigationDestinations
            .singleWhere((item) => item.label == label)
            .destination,
      ),
    ),
  );
}

Widget _shell({
  AppNavigationController? controller,
  Map<AppDestination, Widget>? destinations,
  Map<AppDestination, List<Widget>> headerActions = const {},
  bool unifiedTitleBar = false,
  Future<void> Function()? onCloseWindow,
  Future<void> Function()? onQuit,
  Future<void> Function()? onToggleFullScreen,
  Future<void> Function()? onExitFullScreen,
  VoidCallback? onRefresh,
}) {
  return ShadcnApp(
    theme: AppTheme.light,
    darkTheme: AppTheme.dark,
    themeMode: AppTheme.mode,
    builder: AppTheme.builder,
    home: AppShell(
      controller: controller ?? AppNavigationController(),
      headerActions: headerActions,
      unifiedTitleBar: unifiedTitleBar,
      onCloseWindow: onCloseWindow,
      onQuit: onQuit,
      onToggleFullScreen: onToggleFullScreen,
      onExitFullScreen: onExitFullScreen,
      onRefresh: onRefresh,
      destinations:
          destinations ??
          const {
            AppDestination.history: Text('History body'),
            AppDestination.devices: Text('Devices body'),
            AppDestination.settings: Text('Settings body'),
          },
    ),
  );
}

Widget _testShell() {
  return AppShell(
    controller: AppNavigationController(),
    destinations: const {
      AppDestination.history: SizedBox.expand(),
      AppDestination.devices: SizedBox.expand(),
      AppDestination.settings: SizedBox.expand(),
    },
  );
}

class _RetainedForm extends StatefulWidget {
  const _RetainedForm({
    required this.initializations,
    required this.inputController,
    required this.scrollController,
  });

  final ValueNotifier<int> initializations;
  final TextEditingController inputController;
  final ScrollController scrollController;

  @override
  State<_RetainedForm> createState() => _RetainedFormState();
}

class _RetainedFormState extends State<_RetainedForm> {
  @override
  void initState() {
    super.initState();
    widget.initializations.value++;
  }

  @override
  Widget build(BuildContext context) {
    return ListView(
      key: const ValueKey<String>('retained-form-list'),
      controller: widget.scrollController,
      padding: const EdgeInsets.all(AppSpacing.xxl),
      children: [
        TextField(controller: widget.inputController),
        for (var index = 0; index < 30; index++)
          Padding(
            padding: const EdgeInsets.symmetric(vertical: AppSpacing.xxl),
            child: Text('Item $index'),
          ),
      ],
    );
  }
}
