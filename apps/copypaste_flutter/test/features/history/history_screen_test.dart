import 'dart:async';
import 'dart:typed_data';

import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/app/theme/app_tokens.dart';
import 'package:copypaste_flutter/features/devices/devices_gateway.dart';
import 'package:copypaste_flutter/features/history/controller/history_controller.dart';
import 'package:copypaste_flutter/features/history/models/history_models.dart';
import 'package:copypaste_flutter/features/history/presentation/history_identity_label.dart';
import 'package:copypaste_flutter/features/history/repository/history_file_downloader.dart';
import 'package:copypaste_flutter/features/history/repository/history_repository.dart';
import 'package:copypaste_flutter/features/history/view/history_screen.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('shows the wide inspector only after a clip is selected', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final repository = _ScreenRepository()
      ..page = HistoryClipPage(
        items: [
          HistoryClip(
            id: 'selectable',
            contentType: 'text/plain',
            preview: 'Selectable clip',
            createdAt: DateTime.utc(2026),
            pinned: false,
          ),
        ],
      );
    final controller = HistoryController(repository);
    addTearDown(controller.dispose);

    await tester.pumpWidget(
      ShadcnApp(
        home: Scaffold(child: HistoryScreen(controller: controller)),
      ),
    );
    await tester.pumpAndSettle();

    final list = find.byKey(const ValueKey<String>('history-clip-list'));
    expect(list, findsOneWidget);
    expect(
      find.byKey(const ValueKey<String>('history-detail-inspector')),
      findsNothing,
    );
    expect(find.text('Select a clip'), findsNothing);
    expect(tester.getSize(list).width, closeTo(1400 - AppSpacing.xxxl, 1));

    await tester.tap(find.text('Selectable clip'));
    await tester.pumpAndSettle();

    expect(
      find.byKey(const ValueKey<String>('history-detail-inspector')),
      findsOneWidget,
    );
    expect(tester.getSize(list).width, lessThan(1400 - AppSpacing.xxxl));
  });

  testWidgets('switches from the right inspector below 800 logical pixels', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(800, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final repository = _ScreenRepository()
      ..page = HistoryClipPage(
        items: [
          HistoryClip(
            id: 'breakpoint',
            contentType: 'text/plain',
            preview: 'Breakpoint clip',
            createdAt: DateTime.utc(2026),
            pinned: false,
          ),
        ],
      );
    final controller = HistoryController(repository);
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        home: Scaffold(child: HistoryScreen(controller: controller)),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.text('Breakpoint clip'));
    await tester.pumpAndSettle();

    expect(
      find.byKey(const ValueKey<String>('history-detail-inspector')),
      findsOneWidget,
    );
    expect(
      find.byKey(const ValueKey<String>('history-detail-drawer')),
      findsNothing,
    );

    await tester.binding.setSurfaceSize(const Size(799, 900));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Breakpoint clip'));
    await tester.pumpAndSettle();

    expect(
      find.byKey(const ValueKey<String>('history-detail-inspector')),
      findsNothing,
    );
    expect(
      find.byKey(const ValueKey<String>('history-detail-drawer')),
      findsOneWidget,
    );
    expect(tester.takeException(), isNull);
  });

  testWidgets('resizes the wide inspector from its border without a handle', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final repository = _ScreenRepository()
      ..page = HistoryClipPage(
        items: [
          HistoryClip(
            id: 'text',
            contentType: 'text/plain',
            preview: 'Resizable inspector clip',
            createdAt: DateTime.utc(2026),
            pinned: false,
          ),
        ],
      );
    final controller = HistoryController(repository);
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        home: Scaffold(child: HistoryScreen(controller: controller)),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.text('Resizable inspector clip'));
    await tester.pumpAndSettle();

    expect(find.text('Selected'), findsNothing);
    final selectedCardFinder = find.byKey(
      const ValueKey<String>('history-clip-text'),
    );
    final selectedCard = tester.widget<Button>(selectedCardFinder);
    final selectedCardContext = tester.element(selectedCardFinder);
    final inactiveDecoration = const ButtonStyle.ghost().decoration(
      selectedCardContext,
      const {},
    );
    final activeDecoration = selectedCard.style.decoration(
      selectedCardContext,
      const {},
    );
    expect(activeDecoration, isNot(inactiveDecoration));
    expect((inactiveDecoration as BoxDecoration).border, isNull);
    expect((activeDecoration as BoxDecoration).border, isNull);
    expect(find.byIcon(LucideIcons.gripVertical), findsNothing);
    final inspector = find.byKey(
      const ValueKey<String>('history-detail-inspector'),
    );
    final clipList = find.byKey(const ValueKey<String>('history-clip-list'));
    expect(inspector, findsOneWidget);
    expect(clipList, findsOneWidget);
    final initialRect = tester.getRect(inspector);
    final initialListRect = tester.getRect(clipList);
    expect(
      initialRect.left - initialListRect.right,
      closeTo(AppSpacing.lg, 0.01),
    );

    await tester.dragFrom(
      Offset(initialRect.left, initialRect.center.dy),
      const Offset(-80, 0),
    );
    await tester.pumpAndSettle();

    expect(tester.getSize(inspector).width, greaterThan(initialRect.width));
    expect(
      tester.getRect(inspector).left - tester.getRect(clipList).right,
      closeTo(AppSpacing.lg, 0.01),
    );
  });

  testWidgets(
    'keeps desktop metadata below actions and outside content scrolling',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(1400, 900));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final repository = _ScreenRepository()
        ..page = HistoryClipPage(
          items: [
            HistoryClip(
              id: 'details',
              contentType: 'text/plain',
              preview: 'Desktop metadata clip',
              body: List<String>.filled(
                100,
                'Scrollable desktop detail content',
              ).join('\n'),
              createdAt: DateTime.utc(2026),
              pinned: false,
              sourceApp: 'Editor',
              file: const HistoryFileDetails(
                name: 'notes.txt',
                mimeType: 'text/plain',
                sizeBytes: 2048,
                fileCount: 2,
              ),
            ),
          ],
        );
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      await tester.pumpWidget(
        ShadcnApp(
          home: Scaffold(child: HistoryScreen(controller: controller)),
        ),
      );
      await tester.pumpAndSettle();

      await tester.tap(find.text('Desktop metadata clip'));
      await tester.pumpAndSettle();

      final scrollContent = find.byKey(
        const ValueKey<String>('history-detail-scroll-content'),
      );
      final actions = find.byKey(
        const ValueKey<String>('history-detail-actions'),
      );
      final metadata = find.byKey(
        const ValueKey<String>('history-detail-metadata'),
      );
      expect(scrollContent, findsOneWidget);
      expect(actions, findsOneWidget);
      expect(metadata, findsOneWidget);
      expect(find.byType(OutlineButton), findsNothing);
      expect(find.widgetWithText(Button, 'Copy plain text'), findsOneWidget);
      expect(find.widgetWithText(Button, 'Pin'), findsOneWidget);
      expect(
        find.descendant(of: scrollContent, matching: actions),
        findsNothing,
      );
      expect(
        find.descendant(of: scrollContent, matching: metadata),
        findsNothing,
      );
      expect(
        tester.getBottomLeft(actions).dy,
        lessThan(tester.getTopLeft(metadata).dy),
      );
      final table = tester.widget<Table>(metadata);
      final tableContext = tester.element(metadata);
      final theme = Theme.of(tableContext);
      expect(table.theme, isNull);
      for (final row in table.rows!) {
        final border = row
            .buildDefaultTheme(tableContext)
            .border!
            .resolve(const <WidgetState>{})!;
        expect(border.top.style, BorderStyle.none);
        expect(border.left.style, BorderStyle.none);
        expect(border.right.style, BorderStyle.none);
        expect(border.bottom.color, theme.colorScheme.border);
        expect(border.bottom.width, 1);
      }
      for (final label in [
        'Captured',
        'Source',
        'Clip type',
        'File',
        'Type',
        'Size',
        'Files',
      ]) {
        expect(
          find.descendant(of: metadata, matching: find.text(label)),
          findsOneWidget,
        );
      }
      final sourceApp = find.byKey(
        const ValueKey<String>('history-detail-source-app'),
      );
      expect(sourceApp, findsOneWidget);
      expect(tester.widget<HistoryIdentityLabel>(sourceApp).name, 'Editor');
      expect(
        find.descendant(of: sourceApp, matching: find.byType(Avatar)),
        findsOneWidget,
      );
      expect(
        find.descendant(of: metadata, matching: find.text('notes.txt')),
        findsOneWidget,
      );
      expect(
        find.descendant(of: metadata, matching: find.text('2.0 KB')),
        findsOneWidget,
      );
      expect(
        find.descendant(of: metadata, matching: find.text('Sync')),
        findsNothing,
      );
      expect(
        tester
            .widget<Text>(
              find.descendant(of: metadata, matching: find.text('Captured')),
            )
            .style
            ?.fontSize,
        theme.typography.xSmall.fontSize,
      );
      expect(
        tester.widget<HistoryIdentityLabel>(sourceApp).style?.fontSize,
        theme.typography.xSmall.fontSize,
      );

      final actionsTop = tester.getTopLeft(actions).dy;
      final metadataTop = tester.getTopLeft(metadata).dy;
      final scrollable = find.descendant(
        of: scrollContent,
        matching: find.byType(Scrollable),
      );
      final scrollState = tester.state<ScrollableState>(scrollable.first);
      expect(scrollState.position.maxScrollExtent, greaterThan(0));
      await tester.drag(scrollContent, const Offset(0, -300));
      await tester.pump();

      expect(scrollState.position.pixels, greaterThan(0));
      expect(tester.getTopLeft(actions).dy, actionsTop);
      expect(tester.getTopLeft(metadata).dy, metadataTop);
    },
  );

  testWidgets('places the selected clip kind in the shared control container', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final repository = _ScreenRepository()
      ..page = HistoryClipPage(
        items: [
          HistoryClip(
            id: 'image-kind',
            contentType: 'image/png',
            preview: '[image]',
            createdAt: DateTime.utc(2026),
            pinned: false,
            kind: HistoryClipKind.image,
            origin: 'Work Mac',
            originDeviceClass: DeviceClass.laptop,
            image: const HistoryImageDetails(
              width: 1920,
              height: 1080,
              sizeBytes: 4 * 1024 * 1024,
            ),
            tooLargeToSync: true,
          ),
        ],
      );
    final controller = HistoryController(repository);
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        home: Scaffold(child: HistoryScreen(controller: controller)),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(
      find.byKey(const ValueKey<String>('history-clip-image-kind')),
    );
    await tester.pumpAndSettle();

    final container = find.byKey(
      const ValueKey<String>('history-detail-kind-icon'),
    );
    final card = tester.widget<Card>(
      find.descendant(of: container, matching: find.byType(Card)),
    );
    final theme = Theme.of(tester.element(container));
    expect(tester.getSize(container), const Size.square(AppControlSize.large));
    expect(card.theme?.filled, isTrue);
    expect(card.theme?.fillColor, theme.colorScheme.secondary);
    expect(card.theme?.borderRadius, theme.borderRadiusMd);
    expect(card.theme?.borderWidth, AppSpacing.zero);
    expect(
      tester
          .widget<Icon>(
            find.descendant(
              of: container,
              matching: find.byIcon(LucideIcons.image),
            ),
          )
          .size,
      AppIconSize.sm,
    );
    final metadata = find.byKey(
      const ValueKey<String>('history-detail-metadata'),
    );
    for (final entry in <String, String>{
      'Clip type': 'Image',
      'Type': 'image/png',
      'Resolution': '1920 × 1080',
      'Size': '4.0 MB',
      'Sync': 'Too large to sync',
    }.entries) {
      expect(
        find.descendant(of: metadata, matching: find.text(entry.key)),
        findsOneWidget,
      );
      expect(
        find.descendant(of: metadata, matching: find.text(entry.value)),
        findsOneWidget,
      );
    }
    final device = find.byKey(const ValueKey<String>('history-detail-device'));
    expect(
      find.descendant(of: metadata, matching: find.text('Device')),
      findsOneWidget,
    );
    expect(device, findsOneWidget);
    expect(tester.widget<HistoryIdentityLabel>(device).name, 'Work Mac');
    expect(
      find.descendant(of: device, matching: find.byIcon(LucideIcons.laptop)),
      findsOneWidget,
    );
    expect(
      find.descendant(of: device, matching: find.byType(Avatar)),
      findsNothing,
    );
    final syncWarning = tester.widget<SelectableText>(
      find.descendant(
        of: metadata,
        matching: find.byWidgetPredicate(
          (widget) =>
              widget is SelectableText && widget.data == 'Too large to sync',
        ),
      ),
    );
    expect(syncWarning.style?.color, theme.colorScheme.destructive);
  });

  testWidgets('keeps fitting desktop text at its intrinsic height', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 1100));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final repository = _ScreenRepository()
      ..page = HistoryClipPage(
        items: [
          HistoryClip(
            id: 'short-text',
            contentType: 'text/plain',
            preview: 'shadcn',
            createdAt: DateTime.utc(2026),
            pinned: false,
          ),
        ],
      );
    final controller = HistoryController(repository);
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        home: Scaffold(child: HistoryScreen(controller: controller)),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.text('shadcn'));
    await tester.pumpAndSettle();

    final scrollContent = find.byKey(
      const ValueKey<String>('history-detail-scroll-content'),
    );
    final detailText = find.descendant(
      of: scrollContent,
      matching: find.text('shadcn'),
    );
    final actions = find.byKey(
      const ValueKey<String>('history-detail-actions'),
    );
    expect(detailText, findsOneWidget);
    expect(
      tester.getTopLeft(actions).dy - tester.getBottomLeft(detailText).dy,
      closeTo(AppSpacing.lg, 0.01),
    );
    final scrollable = find.descendant(
      of: scrollContent,
      matching: find.byType(Scrollable),
    );
    expect(
      tester.state<ScrollableState>(scrollable.first).position.maxScrollExtent,
      0,
    );
  });

  testWidgets('keeps a fitting desktop image at its natural preview size', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 1100));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final repository = _ScreenRepository()
      ..availableImagePreview = HistoryImagePreview(
        Uint8List.fromList([0, 1, 2, 3]),
        width: 240,
        height: 90,
      )
      ..page = HistoryClipPage(
        items: [
          HistoryClip(
            id: 'image',
            contentType: 'image/png',
            preview: '[image]',
            createdAt: DateTime.utc(2026),
            pinned: false,
            kind: HistoryClipKind.image,
          ),
        ],
      );
    final controller = HistoryController(repository);
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        home: Scaffold(child: HistoryScreen(controller: controller)),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const ValueKey<String>('history-clip-image')));
    await tester.pumpAndSettle();

    final inspector = find.byKey(
      const ValueKey<String>('history-detail-inspector'),
    );
    final imageViewport = find.byKey(
      const ValueKey<String>('history-detail-image-viewport'),
    );
    final actions = find.byKey(
      const ValueKey<String>('history-detail-actions'),
    );
    expect(imageViewport, findsOneWidget);
    final initialInspectorRect = tester.getRect(inspector);
    final initialViewportSize = tester.getSize(imageViewport);
    expect(initialViewportSize, const Size(240, 90));
    expect(
      tester.getTopLeft(actions).dy - tester.getBottomLeft(imageViewport).dy,
      closeTo(AppSpacing.lg, 0.01),
    );

    await tester.dragFrom(
      Offset(initialInspectorRect.left, initialInspectorRect.center.dy),
      const Offset(-80, 0),
    );
    await tester.pumpAndSettle();

    expect(tester.getSize(imageViewport), initialViewportSize);
  });

  testWidgets('opens a selected clip in a full-width bottom drawer', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(480, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final repository = _ScreenRepository()
      ..page = HistoryClipPage(
        items: [
          HistoryClip(
            id: 'text',
            contentType: 'text/plain',
            preview: 'A selected text clip',
            body: List<String>.filled(
              80,
              'Scrollable detail content',
            ).join('\n'),
            createdAt: DateTime.utc(2026),
            pinned: false,
          ),
        ],
      );
    final controller = HistoryController(repository);
    addTearDown(controller.dispose);
    final drawerVisibility = <bool>[];
    await tester.pumpWidget(
      ShadcnApp(
        home: Scaffold(
          child: SizedBox(
            width: 480,
            height: 800,
            child: HistoryScreen(
              controller: controller,
              onDrawerVisibilityChanged: drawerVisibility.add,
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('A selected text clip'), findsOneWidget);
    await tester.tap(find.text('A selected text clip'));
    await tester.pumpAndSettle();

    final drawer = find.byKey(const ValueKey<String>('history-detail-drawer'));
    expect(drawer, findsOneWidget);
    expect(find.byType(DrawerWrapper), findsOneWidget);
    expect(find.byType(AlertDialog), findsNothing);
    expect(drawerVisibility, [isTrue]);
    final drawerSize = tester.getSize(drawer);
    final mediaSize = MediaQuery.sizeOf(tester.element(drawer));
    expect(drawerSize.width, closeTo(480, 2));
    expect(
      drawerSize.height,
      mediaSize.height * AppOverlaySize.drawerHeightFactor,
    );
    expect(tester.getBottomRight(find.byType(DrawerWrapper)).dy, 800);
    expect(find.text('Copy plain text'), findsOneWidget);
    final detailCard = find.byKey(
      const ValueKey<String>('history-detail-drawer-card'),
    );
    final scrollContent = find.byKey(
      const ValueKey<String>('history-detail-scroll-content'),
    );
    final actions = find.byKey(
      const ValueKey<String>('history-detail-actions'),
    );
    final close = find.byKey(
      const ValueKey<String>('history-detail-drawer-close'),
    );
    expect(find.descendant(of: detailCard, matching: close), findsNothing);
    expect(find.descendant(of: scrollContent, matching: actions), findsNothing);
    final actionsTop = tester.getTopLeft(actions).dy;
    await tester.drag(scrollContent, const Offset(0, -300));
    await tester.pump();
    expect(tester.getTopLeft(actions).dy, actionsTop);

    await tester.tap(close);
    await tester.pumpAndSettle();
    expect(drawer, findsNothing);
    expect(drawerVisibility, [isTrue, isFalse]);
  });

  testWidgets('fits a tall mobile image inside the drawer without scrolling', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(480, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final repository = _ScreenRepository()
      ..availableImagePreview = HistoryImagePreview(
        Uint8List.fromList([0, 1, 2, 3]),
        width: 240,
        height: 1200,
      )
      ..page = HistoryClipPage(
        items: [
          HistoryClip(
            id: 'tall-mobile-image',
            contentType: 'image/png',
            preview: '[image]',
            createdAt: DateTime.utc(2026),
            pinned: false,
            kind: HistoryClipKind.image,
            origin: 'Work Mac',
            originDeviceClass: DeviceClass.laptop,
            sourceApp: 'Editor',
            image: const HistoryImageDetails(
              width: 240,
              height: 1200,
              sizeBytes: 4096,
            ),
            tooLargeToSync: true,
          ),
        ],
      );
    final controller = HistoryController(repository);
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        home: Scaffold(child: HistoryScreen(controller: controller)),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(
      find.byKey(const ValueKey<String>('history-clip-tall-mobile-image')),
    );
    await tester.pumpAndSettle();

    final drawer = find.byKey(const ValueKey<String>('history-detail-drawer'));
    final fittedContent = find.byKey(
      const ValueKey<String>('history-detail-image-fit-content'),
    );
    final imageViewport = find.byKey(
      const ValueKey<String>('history-detail-image-viewport'),
    );
    final actions = find.byKey(
      const ValueKey<String>('history-detail-actions'),
    );
    expect(drawer, findsOneWidget);
    expect(fittedContent, findsOneWidget);
    expect(imageViewport, findsOneWidget);
    final device = find.descendant(
      of: fittedContent,
      matching: find.byKey(const ValueKey<String>('history-detail-device')),
    );
    expect(device, findsOneWidget);
    expect(tester.widget<HistoryIdentityLabel>(device).name, 'Work Mac');
    expect(
      find.descendant(of: device, matching: find.byIcon(LucideIcons.laptop)),
      findsOneWidget,
    );
    expect(
      find.descendant(of: fittedContent, matching: find.byType(Scrollable)),
      findsNothing,
    );
    expect(
      tester.getTopLeft(imageViewport).dy,
      greaterThanOrEqualTo(tester.getTopLeft(fittedContent).dy),
    );
    expect(
      tester.getBottomLeft(imageViewport).dy,
      lessThanOrEqualTo(tester.getBottomLeft(fittedContent).dy),
    );
    expect(
      tester.getBottomLeft(imageViewport).dy,
      lessThan(tester.getTopLeft(actions).dy),
    );
    expect(tester.getSize(imageViewport).height, lessThan(1200));
    expect(
      tester.getBottomLeft(actions).dy,
      lessThanOrEqualTo(tester.getBottomLeft(drawer).dy),
    );

    await tester.binding.setSurfaceSize(const Size(700, 480));
    await tester.pumpAndSettle();

    expect(tester.getTopLeft(drawer).dy, greaterThanOrEqualTo(0));
    expect(tester.getBottomLeft(drawer).dy, lessThanOrEqualTo(480));
    expect(
      tester.getBottomLeft(imageViewport).dy,
      lessThan(tester.getTopLeft(actions).dy),
    );
    expect(
      tester.getBottomLeft(actions).dy,
      lessThanOrEqualTo(tester.getBottomLeft(drawer).dy),
    );
  });

  testWidgets('renders backend facet labels without exposing their IDs', (
    tester,
  ) async {
    final repository = _ScreenRepository()
      ..availableFacets = const HistoryFacets(
        originDevices: [HistoryFilterFacet(id: 'device-1', label: 'Work Mac')],
        sourceApps: [
          HistoryFilterFacet(id: 'com.example.editor', label: 'Editor'),
        ],
      );
    final controller = HistoryController(repository);
    await tester.pumpWidget(
      ShadcnApp(
        home: SizedBox(
          width: 480,
          height: 800,
          child: HistoryScreen(controller: controller),
        ),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.bySemanticsLabel('All devices'));
    await tester.pump(const Duration(milliseconds: 200));
    expect(find.text('Work Mac'), findsOneWidget);
    expect(find.text('device-1'), findsNothing);
    expect(find.text('com.example.editor'), findsNothing);
    await tester.tap(find.text('Work Mac'));
    await tester.pumpAndSettle();

    await tester.tap(find.bySemanticsLabel('All apps'));
    await tester.pump(const Duration(milliseconds: 200));
    expect(find.text('Editor'), findsOneWidget);
    expect(find.text('com.example.editor'), findsNothing);
    controller.dispose();
  });

  testWidgets('shows skipped rows when an empty history page is unreadable', (
    tester,
  ) async {
    final repository = _ScreenRepository()
      ..page = const HistoryClipPage(items: [], skippedUndecryptable: 2);
    final controller = HistoryController(repository);
    await tester.pumpWidget(
      ShadcnApp(
        home: SizedBox(
          width: 480,
          height: 800,
          child: HistoryScreen(controller: controller),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('2 clipboard rows were skipped'), findsOneWidget);
    expect(find.text('No clips found'), findsOneWidget);
    controller.dispose();
  });

  testWidgets('keeps every compact icon-only toolbar control square', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(390, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final controller = HistoryController(_ScreenRepository());
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        darkTheme: AppTheme.dark,
        themeMode: ThemeMode.light,
        builder: AppTheme.builder,
        home: HistoryScreen(controller: controller),
      ),
    );
    await tester.pumpAndSettle();

    final searchButton = find.byKey(
      const ValueKey<String>('history-search-toggle'),
    );
    final controls = <Element>[
      ...searchButton.evaluate(),
      ...find.byWidgetPredicate((widget) => widget is Select).evaluate(),
    ];
    expect(searchButton, findsOneWidget);
    expect(
      find.byWidgetPredicate((widget) => widget is Select),
      findsNWidgets(5),
    );
    for (final control in controls) {
      final size = tester.getSize(find.byWidget(control.widget));
      expect(size, const Size.square(40));
    }

    final search = tester.widget<Button>(searchButton);
    final searchContext = tester.element(searchButton);
    final searchDecoration =
        search.style.decoration(searchContext, const {}) as BoxDecoration;
    final firstSelectFinder = find
        .byWidgetPredicate((widget) => widget is Select)
        .first;
    final firstSelect = tester.widget<Select<dynamic>>(firstSelectFinder);
    final selectDecoration =
        firstSelect.theme!.decoration!(
              tester.element(firstSelectFinder),
              const {},
              const BoxDecoration(),
            )
            as BoxDecoration;
    expect(searchDecoration.color, selectDecoration.color);
    expect(searchDecoration.borderRadius, selectDecoration.borderRadius);
  });

  testWidgets('keeps the expanded search field at least 160 pixels wide', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(432, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final controller = HistoryController(_ScreenRepository());
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        home: SizedBox(
          width: 432,
          height: 800,
          child: HistoryScreen(controller: controller),
        ),
      ),
    );
    await tester.pumpAndSettle();

    final searchField = find.byType(TextField);
    expect(searchField, findsOneWidget);
    expect(tester.getSize(searchField).width, 160);
  });

  testWidgets('shows only the standardized kind icon below the breakpoint', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(799, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final repository = _ScreenRepository()
      ..page = HistoryClipPage(
        items: [
          HistoryClip(
            id: 'meta',
            contentType: 'text',
            preview: 'Content stays at its normal size',
            createdAt: DateTime.utc(2026),
            pinned: true,
            kind: HistoryClipKind.text,
            sourceApp: 'Editor',
          ),
        ],
      );
    final controller = HistoryController(repository);
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        home: SizedBox(
          width: 799,
          height: 800,
          child: HistoryScreen(controller: controller),
        ),
      ),
    );
    await tester.pumpAndSettle();

    final metaFinder = find.byKey(
      const ValueKey<String>('history-clip-meta-meta'),
    );
    final meta = tester.widget<Text>(metaFinder);
    final metaTheme = Theme.of(tester.element(metaFinder));
    expect(meta.textSpan?.toPlainText(), isNot(contains('Text')));
    final kindIcon = find.descendant(
      of: metaFinder,
      matching: find.byIcon(LucideIcons.type),
    );
    expect(kindIcon, findsOneWidget);
    expect(tester.widget<Icon>(kindIcon).size, AppIconSize.xs);
    expect(meta.style?.fontSize, metaTheme.typography.xSmall.fontSize);
    expect(
      tester.getSize(find.byType(Avatar)),
      const Size.square(AppIconSize.sm),
    );
    expect(
      tester.widget<Icon>(find.byIcon(LucideIcons.pin)).size,
      AppIconSize.xs,
    );
  });

  testWidgets('shows the standardized kind icon and label at the wide size', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final repository = _ScreenRepository()
      ..page = HistoryClipPage(
        items: [
          HistoryClip(
            id: 'wide-meta',
            contentType: 'text',
            preview: 'Desktop clip content',
            createdAt: DateTime.utc(2026),
            pinned: false,
            kind: HistoryClipKind.text,
          ),
        ],
      );
    final controller = HistoryController(repository);
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        home: Scaffold(child: HistoryScreen(controller: controller)),
      ),
    );
    await tester.pumpAndSettle();

    final metaFinder = find.byKey(
      const ValueKey<String>('history-clip-meta-wide-meta'),
    );
    final meta = tester.widget<Text>(metaFinder);
    expect(meta.textSpan?.toPlainText(), contains('Text'));
    expect(
      find.descendant(of: metaFinder, matching: find.byIcon(LucideIcons.type)),
      findsOneWidget,
    );
  });

  testWidgets('offers every semantic kind and sends the selected filter', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(2400, 1000));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final repository = _ScreenRepository();
    final controller = HistoryController(repository);
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        home: Scaffold(child: HistoryScreen(controller: controller)),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.text('All clips'));
    await tester.pump(const Duration(milliseconds: 200));

    for (final label in ['Text', 'Link', 'Email', 'Color', 'Phone', 'Code']) {
      expect(find.text(label), findsOneWidget, reason: label);
    }

    await tester.tap(find.text('Link'));
    await tester.pump(const Duration(milliseconds: 200));
    expect(repository.queries.last.kind, HistoryClipKind.link);
  });

  testWidgets('facet dropdowns use no Radix overflow icons', (tester) async {
    final originDevices = List<HistoryFilterFacet>.generate(
      40,
      (index) =>
          HistoryFilterFacet(id: 'device-$index', label: 'Device $index'),
    );
    final sourceApps = List<HistoryFilterFacet>.generate(
      40,
      (index) => HistoryFilterFacet(id: 'app-$index', label: 'App $index'),
    );
    final repository = _ScreenRepository()
      ..availableFacets = HistoryFacets(
        originDevices: originDevices,
        sourceApps: sourceApps,
      );
    final controller = HistoryController(repository);
    await tester.pumpWidget(
      ShadcnApp(
        home: Scaffold(
          child: SizedBox(
            width: 480,
            height: 800,
            child: HistoryScreen(controller: controller),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.bySemanticsLabel('All devices'));
    await tester.pump(const Duration(milliseconds: 200));
    _expectNoRadixIcons(tester);
    await tester.tap(find.text('Device 0'));
    await tester.pumpAndSettle();

    await tester.tap(find.bySemanticsLabel('All apps'));
    await tester.pump(const Duration(milliseconds: 200));
    _expectNoRadixIcons(tester);
    controller.dispose();
  });

  testWidgets('all history filters use soft borderless select triggers', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1200, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final controller = HistoryController(_ScreenRepository());
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        home: Scaffold(child: HistoryScreen(controller: controller)),
      ),
    );
    await tester.pumpAndSettle();

    final selectFinder = find.byWidgetPredicate(
      (widget) => widget is Select<dynamic>,
    );
    expect(selectFinder, findsNWidgets(5));
    for (final select in tester.widgetList<Select<dynamic>>(selectFinder)) {
      expect(select.filled, isTrue);
      final selectContext = tester.element(find.byWidget(select));
      for (final states in <Set<WidgetState>>[
        const {},
        const {WidgetState.hovered},
        const {WidgetState.focused},
        const {WidgetState.pressed},
      ]) {
        final decoration =
            select.theme!.decoration!(
                  selectContext,
                  states,
                  const BoxDecoration(),
                )
                as BoxDecoration;
        expect(decoration.border?.top.style, BorderStyle.none);
      }
    }
  });

  testWidgets('offers Download for a file whose source is unavailable', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final file = HistoryClip(
      id: 'remote-file',
      contentType: 'file',
      preview: '/Users/person/Documents/report.pdf',
      body: '/Users/person/Documents/report.pdf',
      createdAt: DateTime.utc(2026),
      pinned: false,
      file: const HistoryFileDetails(
        name: 'report.pdf',
        mimeType: 'application/pdf',
        sourceReference: '/Users/person/Documents/report.pdf',
        sourceAvailable: false,
        sizeBytes: 42,
        fileCount: 1,
      ),
    );
    final repository = _ScreenRepository()
      ..page = HistoryClipPage(items: [file]);
    final downloader = _ScreenFileDownloader();
    final controller = HistoryController(
      repository,
      fileDownloader: downloader,
    );
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        home: Scaffold(child: HistoryScreen(controller: controller)),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.text('/Users/person/Documents/report.pdf').first);
    await tester.pumpAndSettle();
    expect(find.widgetWithText(Button, 'Download'), findsOneWidget);
    await tester.tap(find.widgetWithText(Button, 'Download'));
    await tester.pumpAndSettle();

    expect(repository.savedFiles, [('remote-file', '/tmp/report.pdf')]);
    expect(downloader.presented, ['/tmp/report.pdf']);
    await tester.pump(const Duration(seconds: 6));
  });
}

void _expectNoRadixIcons(WidgetTester tester) {
  final radixIcons = tester
      .widgetList<Icon>(find.byType(Icon))
      .where((icon) => icon.icon?.fontFamily == 'RadixIcons');
  expect(radixIcons, isEmpty);
}

class _ScreenRepository implements HistoryRepository {
  HistoryClipPage page = const HistoryClipPage(items: []);
  HistoryFacets availableFacets = const HistoryFacets();
  final List<(String, String)> savedFiles = [];
  HistoryImagePreview? availableImagePreview;
  final List<HistoryQuery> queries = [];

  @override
  Stream<HistoryRuntimeEvent> watch() => const Stream.empty();

  @override
  Future<HistoryFacets> facets() async => availableFacets;

  @override
  Future<HistoryClipPage> query({
    required HistoryQuery query,
    required int limit,
    String? cursor,
  }) async {
    queries.add(query);
    return page;
  }

  @override
  Future<HistoryClip> get(String id) async => page.items.single;

  @override
  Future<HistoryImagePreview?> imagePreview(String id, {int? maxEdge}) async =>
      availableImagePreview;

  @override
  Future<HistorySourceAppIcon?> sourceAppIcon(String id) async => null;

  @override
  Future<void> copy(String id) async {}

  @override
  Future<void> copyPlainText(String id) async {}

  @override
  Future<void> saveFile(String id, String destinationPath) async {
    savedFiles.add((id, destinationPath));
  }

  @override
  Future<void> delete(String id) async {}

  @override
  Future<void> deleteAll() async {}

  @override
  Future<void> reorderPinned(List<String> ids) async {}

  @override
  Future<void> setPinned(String id, bool pinned) async {}
}

class _ScreenFileDownloader implements HistoryFileDownloader {
  final List<String> presented = [];

  @override
  Future<String?> chooseDestination(HistoryFileDetails file) async =>
      '/tmp/report.pdf';

  @override
  Future<void> presentSavedFile(String path, HistoryFileDetails file) async {
    presented.add(path);
  }
}
