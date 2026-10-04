import 'package:copypaste_flutter/app/navigation/navigation.dart';
import 'package:copypaste_flutter/app/shell/shell.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/app/theme/app_tokens.dart';
import 'package:copypaste_flutter/features/devices/device_presentation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';
import 'package:window_manager/window_manager.dart';

void main() {
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
    expect(_bottomNavigationLabel('History'), findsOneWidget);
    expect(_bottomNavigationLabel('Devices'), findsOneWidget);
    expect(_bottomNavigationLabel('Settings'), findsOneWidget);
    await tester.tap(_bottomNavigationLabel('Devices'));
    await tester.pump();
    expect(find.text('Devices body'), findsOneWidget);
  });

  testWidgets('renders a full-width themed mobile navigation dock', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(442, 720));
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
    final theme = Theme.of(tester.element(dock));

    expect(
      navigationBar.selectedKey,
      const ValueKey<AppDestination>(AppDestination.history),
    );
    expect(navigationBar.labelType, NavigationLabelType.all);
    expect(navigationBar.labelPosition, NavigationLabelPosition.bottom);
    expect(navigationBar.alignment, NavigationBarAlignment.spaceEvenly);
    expect(navigationBar.backgroundColor, theme.colorScheme.secondary);
    expect(
      navigationBar.padding,
      const EdgeInsets.symmetric(
        horizontal: AppSpacing.lg,
        vertical: AppSpacing.sm,
      ),
    );
    expect(navigationBar.spacing, AppSpacing.sm);
    expect(dockBounds.left, 0);
    expect(dockBounds.width, 442);
    expect(bounds.width, 442);
    expect(
      find.descendant(of: dock, matching: find.byType(Card)),
      findsNothing,
    );
    expect(navigationBar.children, hasLength(3));
    final items = navigationBar.children.cast<NavigationItem>();
    expect(items.map((item) => (item.label! as Text).data), [
      'History',
      'Devices',
      'Settings',
    ]);
    expect(items.map((item) => (item.child as Icon).icon), [
      LucideIcons.history,
      DevicePresentation.collectionIcon,
      LucideIcons.settings,
    ]);
    final selectedItem = find.descendant(
      of: dock,
      matching: find.byKey(
        const ValueKey<AppDestination>(AppDestination.history),
      ),
    );
    final selectedDecoration = items.first.selectedStyle!.decoration(
      tester.element(selectedItem),
      const {WidgetState.selected},
    );
    expect(
      (selectedDecoration as BoxDecoration).color,
      theme.colorScheme.accent,
    );
    expect(
      items.first.selectedStyle!.iconTheme(tester.element(selectedItem), const {
        WidgetState.selected,
      }).color,
      AppTheme.navigationAccent,
    );
  });

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

  testWidgets('uses the shadcn navigation rail at the compact breakpoint', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(640, 720));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    await tester.pumpWidget(_shell());

    expect(find.byType(NavigationRail), findsOneWidget);
    expect(find.byType(Tooltip), findsNWidgets(3));
    final tooltips = tester.widgetList<Tooltip>(find.byType(Tooltip));
    for (final tooltip in tooltips) {
      expect(tooltip.alignment, Alignment.topCenter);
      expect(tooltip.anchorAlignment, Alignment.bottomCenter);
      expect(
        tooltip.tooltip(tester.element(find.byType(Tooltip).first)),
        isA<TooltipContainer>(),
      );
    }
    final railCenter = tester.getCenter(find.byType(NavigationRail)).dx;
    for (final icon in [
      LucideIcons.history,
      DevicePresentation.collectionIcon,
      LucideIcons.settings,
    ]) {
      final iconFinder = find.byIcon(icon);
      expect(tester.getCenter(iconFinder).dx, closeTo(railCenter, 1));
      expect(tester.widget<Icon>(iconFinder).size, AppIconSize.md);
    }
    final compactLogo = tester.widget<Image>(
      find.byKey(const ValueKey<String>('copypaste-brand-logo')),
    );
    expect(compactLogo.width, AppIconSize.xl);
    expect(compactLogo.height, AppIconSize.xl);
    expect(
      tester
          .getCenter(find.byKey(const ValueKey<String>('copypaste-brand-logo')))
          .dx,
      closeTo(railCenter, 1),
    );
    await tester.tap(find.byIcon(DevicePresentation.collectionIcon));
    await tester.pump();
    expect(find.text('Devices body'), findsOneWidget);
  });

  testWidgets('uses the shadcn expandable rail at the expanded breakpoint', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1024, 720));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    await tester.pumpWidget(_shell());

    expect(find.byType(NavigationRail), findsOneWidget);
    expect(find.byType(NavigationItem), findsNWidgets(3));
    final expandedRail = tester.widget<NavigationRail>(
      find.byType(NavigationRail),
    );
    expect(
      expandedRail,
      isA<NavigationRail>()
          .having((rail) => rail.expanded, 'expanded', isTrue)
          .having((rail) => rail.expandedSize, 'expandedSize', 220),
    );
    expect(find.text('Devices'), findsOneWidget);
    expect(find.text('Settings'), findsOneWidget);
    expect(
      find.byKey(const ValueKey<String>('navigation-rail-toggle')),
      findsOneWidget,
    );
    expect(
      find.byKey(const ValueKey<String>('navigation-brand')),
      findsOneWidget,
    );
    expect(
      tester
          .getSize(find.byKey(const ValueKey<String>('navigation-brand')))
          .height,
      AppIconSize.xl,
    );
    expect(
      find.byKey(const ValueKey<String>('copypaste-brand-logo')),
      findsOneWidget,
    );
    final expandedLogo = tester.widget<Image>(
      find.byKey(const ValueKey<String>('copypaste-brand-logo')),
    );
    expect(expandedLogo.width, AppIconSize.xl);
    expect(expandedLogo.height, AppIconSize.xl);
    final rail = find.byType(NavigationRail);
    final theme = Theme.of(tester.element(rail));
    expect(expandedRail.backgroundColor, theme.colorScheme.secondary);
    final selectedNavigationItem = tester.widget<NavigationItem>(
      find.descendant(
        of: rail,
        matching: find.byKey(
          const ValueKey<AppDestination>(AppDestination.history),
        ),
      ),
    );
    final selectedDecoration = selectedNavigationItem.selectedStyle!.decoration(
      tester.element(rail),
      const {WidgetState.selected},
    );
    expect(
      (selectedDecoration as BoxDecoration).color,
      theme.colorScheme.accent,
    );
    final navigationItemContext = tester.element(
      find.descendant(
        of: rail,
        matching: find.byKey(
          const ValueKey<AppDestination>(AppDestination.history),
        ),
      ),
    );
    expect(
      selectedNavigationItem.selectedStyle!.padding(
        navigationItemContext,
        const {WidgetState.selected},
      ),
      selectedNavigationItem.style!.padding(navigationItemContext, const {}),
    );
    final brandLabel = find.descendant(
      of: rail,
      matching: find.text('CopyPaste'),
    );
    expect(
      tester
          .getCenter(find.byKey(const ValueKey<String>('copypaste-brand-logo')))
          .dy,
      closeTo(tester.getCenter(brandLabel).dy, 0.5),
    );
    for (final label in ['CopyPaste', 'History', 'Devices', 'Settings']) {
      final labelFinder = find.descendant(of: rail, matching: find.text(label));
      final text = tester.widget<Text>(labelFinder);
      expect(text.style?.fontSize, theme.typography.large.fontSize);
      expect(text.style?.fontWeight, theme.typography.medium.fontWeight);
      expect(text.style?.height, 1);
      if (label == 'CopyPaste') {
        expect(text.style?.color, theme.colorScheme.foreground);
      }
    }
    for (final icon in [
      LucideIcons.history,
      DevicePresentation.collectionIcon,
      LucideIcons.settings,
    ]) {
      expect(tester.widget<Icon>(find.byIcon(icon)).size, AppIconSize.md);
    }
    for (final destination in AppDestination.values) {
      final iconWrapper = find.byKey(
        ValueKey<String>('navigation-icon-${destination.name}'),
      );
      final label = find.descendant(
        of: rail,
        matching: find.text(destination.navigationDestination.label),
      );
      expect(tester.getSize(iconWrapper), const Size.square(AppIconSize.xl));
      expect(
        tester.getCenter(iconWrapper).dy,
        closeTo(tester.getCenter(label).dy, 0.5),
      );
    }
    final railBounds = tester.getRect(find.byType(NavigationRail));
    expect(railBounds.width, 220);
    final toggleBounds = tester.getRect(
      find.byKey(const ValueKey<String>('navigation-rail-toggle')),
    );
    expect(toggleBounds.left, greaterThanOrEqualTo(railBounds.right));
    await tester.tap(find.byIcon(LucideIcons.settings));
    await tester.pump();
    expect(find.text('Settings body'), findsOneWidget);
    expect(
      tester.widget<NavigationRail>(find.byType(NavigationRail)).expanded,
      isFalse,
    );
  });

  testWidgets('collapsing the rail changes only horizontal geometry', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1024, 720));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    await tester.pumpWidget(_shell());

    final rail = find.byType(NavigationRail);
    final logo = find.byKey(const ValueKey<String>('copypaste-brand-logo'));
    final beforeRailWidth = tester.getSize(rail).width;
    final beforeLogo = tester.getRect(logo);
    final beforeItems = <AppDestination, Rect>{
      for (final destination in AppDestination.values)
        destination: tester.getRect(
          find.descendant(
            of: rail,
            matching: find.byKey(ValueKey<AppDestination>(destination)),
          ),
        ),
    };

    await tester.tap(
      find.byKey(const ValueKey<String>('navigation-rail-toggle')),
    );
    await tester.pumpAndSettle();

    expect(tester.getSize(rail).width, lessThan(beforeRailWidth));
    final afterLogo = tester.getRect(logo);
    expect(afterLogo.height, beforeLogo.height);
    expect(afterLogo.center.dy, closeTo(beforeLogo.center.dy, 0.5));
    for (final destination in AppDestination.values) {
      final after = tester.getRect(
        find.descendant(
          of: rail,
          matching: find.byKey(ValueKey<AppDestination>(destination)),
        ),
      );
      expect(after.height, beforeItems[destination]!.height);
      expect(
        after.center.dy,
        closeTo(beforeItems[destination]!.center.dy, 0.5),
      );
    }
  });

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
    'places the unified macOS header above navigation with a native control reserve',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(1200, 720));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final controller = AppNavigationController();
      addTearDown(controller.dispose);

      await tester.pumpWidget(
        _shell(
          controller: controller,
          unifiedTitleBar: true,
          headerActions: const {
            AppDestination.devices: [Text('Device header actions')],
          },
        ),
      );

      final header = find.byType(MacosWindowHeader);
      final reserve = find.byKey(
        const ValueKey<String>('macos-native-controls-reserve'),
      );
      expect(header, findsOneWidget);
      expect(find.byType(DragToMoveArea), findsOneWidget);
      expect(
        find.byKey(const ValueKey<String>('macos-window-drag-region')),
        findsOneWidget,
      );
      expect(
        tester.getSize(reserve).width,
        MacosWindowHeader.nativeControlsWidth,
      );
      expect(tester.getSize(header).height, AppControlSize.touch);
      final headerAppBar = tester.widget<AppBar>(
        find.descendant(of: header, matching: find.byType(AppBar)),
      );
      final headerTheme = Theme.of(tester.element(header));
      expect(headerAppBar.backgroundColor, headerTheme.colorScheme.muted);
      expect(
        tester.getRect(header).bottom,
        lessThanOrEqualTo(tester.getRect(find.byType(NavigationRail)).top),
      );
      final headerTitle = find.descendant(
        of: header,
        matching: find.text('History'),
      );
      expect(headerTitle, findsOneWidget);
      expect(
        tester.getCenter(headerTitle).dy,
        closeTo(tester.getCenter(header).dy, 0.5),
      );
      expect(DefaultTextStyle.of(tester.element(headerTitle)).style.height, 1);
      expect(
        find.byKey(const ValueKey<String>('navigation-rail-toggle')),
        findsOneWidget,
      );

      controller.selectDestination(AppDestination.devices);
      await tester.pump();

      expect(
        find.descendant(of: header, matching: find.text('Devices')),
        findsOneWidget,
      );
      expect(find.text('Device header actions'), findsOneWidget);
    },
  );

  testWidgets('keeps the standard content header by default', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1200, 720));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    await tester.pumpWidget(_shell());

    expect(find.byType(MacosWindowHeader), findsNothing);
    expect(find.byType(DragToMoveArea), findsNothing);
    expect(
      find.ancestor(of: find.text('History'), matching: find.byType(AppBar)),
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

      final navigationBar = tester.widget<NavigationBar>(
        find.byKey(const ValueKey<String>('bottom-navigation')),
      );
      expect(
        ((navigationBar.children.last as NavigationItem).label! as Text).data,
        'Settings',
      );
      expect(_bottomNavigationLabel('History'), findsOneWidget);
      expect(_bottomNavigationLabel('Devices'), findsOneWidget);
      expect(_bottomNavigationLabel('Settings'), findsOneWidget);
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
    await tester.tap(
      find.byKey(const ValueKey<String>('navigation-rail-toggle')),
    );
    await tester.pump();
    expect(
      tester.widget<NavigationRail>(find.byType(NavigationRail)).expanded,
      isFalse,
    );
    expect(
      tester
          .getRect(find.byKey(const ValueKey<String>('navigation-rail-toggle')))
          .left,
      greaterThanOrEqualTo(tester.getRect(find.byType(NavigationRail)).right),
    );
    expect(find.text('Detail route'), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    expect(find.text('Detail route'), findsNothing);
    await detailRoute;
    await tester.binding.setSurfaceSize(const Size(320, 720));
    await tester.pump();
    await tester.binding.setSurfaceSize(const Size(1200, 720));
    await tester.pump();

    expect(
      tester.widget<NavigationRail>(find.byType(NavigationRail)).expanded,
      isFalse,
    );
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
    expect(find.byType(NavigationRail), findsOneWidget);
    tester.view.resetViewInsets();
    await tester.pump();
    expect(find.text('Save'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });
}

Finder _bottomNavigationLabel(String label) {
  return find.descendant(
    of: find.byKey(const ValueKey<String>('bottom-navigation')),
    matching: find.text(label),
  );
}

Widget _shell({
  AppNavigationController? controller,
  Map<AppDestination, Widget>? destinations,
  Map<AppDestination, List<Widget>> headerActions = const {},
  bool unifiedTitleBar = false,
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
