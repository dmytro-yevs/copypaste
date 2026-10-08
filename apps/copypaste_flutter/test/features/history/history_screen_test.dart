import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';

import 'package:copypaste_flutter/features/history/repository/history_file_importer.dart';
import 'package:flutter/gestures.dart';

import 'package:copypaste_flutter/app/theme/app_motion.dart';
import 'package:copypaste_flutter/app/navigation/navigation.dart';
import 'package:copypaste_flutter/app/shell/app_shell.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/app/theme/app_tokens.dart';
import 'package:copypaste_flutter/features/devices/device_label.dart';
import 'package:copypaste_flutter/features/devices/devices_gateway.dart';
import 'package:copypaste_flutter/features/history/controller/history_controller.dart';
import 'package:copypaste_flutter/features/history/models/history_models.dart';
import 'package:copypaste_flutter/features/history/presentation/source_app_label.dart';
import 'package:copypaste_flutter/features/history/repository/history_file_downloader.dart';
import 'package:copypaste_flutter/features/history/repository/history_repository.dart';
import 'package:copypaste_flutter/features/history/view/history_screen.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:image/image.dart' as raster;

void main() {
  testWidgets(
    'mobile history rows and errors clear the floating navigation',
    (tester) async {
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.binding.setSurfaceSize(const Size(320, 720));
      for (final textScale in [1.0, 2.0]) {
        final repository = _ScreenRepository()
          ..failPinUpdate = true
          ..page = HistoryClipPage(
            items: [
              for (var index = 0; index < 30; index++)
                HistoryClip(
                  id: 'clearance-$index',
                  contentType: 'text',
                  preview: 'Clearance clip $index',
                  createdAt: DateTime.utc(2026, 10, 8, 0, 0, 30 - index),
                  pinned: false,
                ),
            ],
          );
        final controller = HistoryController(repository);
        final navigation = AppNavigationController();
        addTearDown(controller.dispose);
        addTearDown(navigation.dispose);
        await controller.initialize();
        await tester.pumpWidget(
          ShadcnApp(
            theme: AppTheme.light,
            builder: (context, child) => AppTheme.builder(
              context,
              MediaQuery(
                data: MediaQuery.of(context).copyWith(
                  padding: const EdgeInsets.only(bottom: 24),
                  viewPadding: const EdgeInsets.only(bottom: 24),
                  textScaler: TextScaler.linear(textScale),
                ),
                child: child!,
              ),
            ),
            home: AppShell(
              controller: navigation,
              destinations: {
                AppDestination.history: HistoryScreen(controller: controller),
                AppDestination.devices: const SizedBox.expand(),
                AppDestination.settings: const SizedBox.expand(),
              },
            ),
          ),
        );
        await tester.pumpAndSettle();
        final list = find.byType(ListView);
        await tester.drag(list, const Offset(0, -10000));
        await tester.pumpAndSettle();
        final position = navigation
            .scrollControllerFor(AppDestination.history)
            .position;
        position.jumpTo(position.maxScrollExtent);
        await tester.pumpAndSettle();
        final dockTop = tester
            .getRect(find.byKey(const ValueKey('mobile-navigation-dock')))
            .top;
        expect(
          tester.getRect(find.text('Clearance clip 29')).bottom,
          lessThan(dockTop),
        );
        await controller.togglePin(controller.items.last);
        await tester.pumpAndSettle();
        position.jumpTo(position.maxScrollExtent);
        await tester.pumpAndSettle();
        expect(tester.getRect(find.byType(Alert)).bottom, lessThan(dockTop));
        await tester.scrollUntilVisible(
          find.text('Clearance clip 29'),
          -200,
          scrollable: find
              .descendant(of: list, matching: find.byType(Scrollable))
              .first,
        );
        await tester.pumpAndSettle();
        expect(
          tester.getCenter(find.text('Clearance clip 29')).dy,
          lessThan(dockTop),
        );
        expect(find.text('Clearance clip 29').hitTestable(), findsOneWidget);
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox.shrink());
      }
    },
    variant: TargetPlatformVariant({
      TargetPlatform.android,
      TargetPlatform.macOS,
      TargetPlatform.windows,
    }),
  );

  testWidgets(
    'overlays compact pin and delete actions on mouse hover',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(1400, 900));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final repository = _ScreenRepository()
        ..page = HistoryClipPage(
          items: [
            HistoryClip(
              id: 'actions',
              contentType: 'text',
              preview: 'Hover action clip',
              createdAt: DateTime.utc(2026),
              pinned: false,
            ),
          ],
        );
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      await tester.pumpWidget(
        ShadcnApp(
          theme: AppTheme.light,
          darkTheme: AppTheme.dark,
          builder: AppTheme.builder,
          home: Scaffold(child: HistoryScreen(controller: controller)),
        ),
      );
      await _pumpHoverActions(tester);
      final clip = find.byKey(const ValueKey<String>('history-clip-actions'));
      final pin = find.byKey(const ValueKey<String>('history-row-pin-actions'));
      final delete = find.byKey(
        const ValueKey<String>('history-row-delete-actions'),
      );
      final rect = tester.getRect(clip);
      expect(pin, findsNothing);
      expect(delete, findsNothing);

      final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
      await mouse.addPointer(location: Offset.zero);
      addTearDown(mouse.removePointer);
      await mouse.moveTo(rect.center);
      await _pumpHoverActions(tester);
      expect(pin, findsOneWidget);
      expect(delete, findsOneWidget);
      expect(tester.getSize(pin), const Size.square(24));
      expect(tester.getSize(delete), const Size.square(24));
      expect(tester.getRect(clip), rect);
      expect(rect.contains(tester.getCenter(delete)), isTrue);

      await mouse.moveTo(Offset.zero);
      await _pumpHoverActions(tester);
      expect(pin, findsNothing);
      expect(delete, findsNothing);
      await mouse.moveTo(rect.center);
      await _pumpHoverActions(tester);

      await mouse.moveTo(tester.getCenter(pin));
      await _pumpHoverActions(tester);
      expect(pin, findsOneWidget);
      await tester.tap(pin, kind: PointerDeviceKind.mouse);
      await _pumpHoverActions(tester);
      expect(repository.pinnedUpdates, [('actions', true)]);
      expect(controller.selectedId, isNull);
      await mouse.moveTo(tester.getCenter(clip));
      await _pumpHoverActions(tester);
      expect(find.byIcon(LucideIcons.pinOff), findsWidgets);
      await tester.tap(pin, kind: PointerDeviceKind.mouse);
      await _pumpHoverActions(tester);
      expect(repository.pinnedUpdates, [('actions', true), ('actions', false)]);

      await mouse.moveTo(tester.getCenter(clip));
      await _pumpHoverActions(tester);
      await tester.tap(delete, kind: PointerDeviceKind.mouse);
      await _pumpHoverActions(tester);
      expect(find.text('Delete this clip?'), findsOneWidget);
      expect(repository.deletedIds, isEmpty);
      await tester.tap(
        find.widgetWithText(Button, 'Cancel'),
        kind: PointerDeviceKind.mouse,
      );
      await _pumpHoverActions(tester);
      expect(repository.deletedIds, isEmpty);

      await mouse.moveTo(tester.getCenter(clip));
      await _pumpHoverActions(tester);
      await tester.tap(delete, kind: PointerDeviceKind.mouse);
      await _pumpHoverActions(tester);
      await tester.tap(
        find.widgetWithText(Button, 'Delete'),
        kind: PointerDeviceKind.mouse,
      );
      await _pumpHoverActions(tester);
      expect(repository.deletedIds, ['actions']);
      expect(controller.items, isEmpty);
      expect(controller.selectedId, isNull);
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant({
      TargetPlatform.macOS,
      TargetPlatform.windows,
    }),
  );

  testWidgets(
    'keeps row actions hidden for touch input',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(1400, 900));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final repository = _ScreenRepository()
        ..page = HistoryClipPage(
          items: [
            HistoryClip(
              id: 'touch',
              contentType: 'text',
              preview: 'Touch action clip',
              createdAt: DateTime.utc(2026),
              pinned: false,
            ),
          ],
        );
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      await tester.pumpWidget(
        ShadcnApp(
          theme: AppTheme.light,
          darkTheme: AppTheme.dark,
          builder: AppTheme.builder,
          home: Scaffold(child: HistoryScreen(controller: controller)),
        ),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.text('Touch action clip'));
      await tester.pumpAndSettle();
      expect(
        find.byKey(const ValueKey<String>('history-row-pin-touch')),
        findsNothing,
      );
      expect(
        find.byKey(const ValueKey<String>('history-row-delete-touch')),
        findsNothing,
      );
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant({
      TargetPlatform.macOS,
      TargetPlatform.windows,
      TargetPlatform.android,
    }),
  );

  for (final scenario in [
    (platform: TargetPlatform.android, width: 480.0, pixelRatio: 1.0),
    (platform: TargetPlatform.windows, width: 900.0, pixelRatio: 2.0),
    (platform: TargetPlatform.macOS, width: 1400.0, pixelRatio: 2.0),
  ]) {
    for (final dimensions in [
      (1200, 200),
      (200, 1200),
      (1200, 1200),
      (60, 30),
    ]) {
      testWidgets(
        'fits ${dimensions.$1}x${dimensions.$2} list previews on ${scenario.platform.name}',
        (tester) async {
          await tester.binding.setSurfaceSize(Size(scenario.width, 900));
          addTearDown(() => tester.binding.setSurfaceSize(null));
          final repository = _ScreenRepository()
            ..fitImagePreviewBounds = true
            ..availableImagePreview = HistoryImagePreview(
              raster.encodePng(
                raster.Image(width: dimensions.$1, height: dimensions.$2),
              ),
              width: dimensions.$1,
              height: dimensions.$2,
            )
            ..page = HistoryClipPage(
              items: [
                HistoryClip(
                  id: 'proportional',
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
              theme: AppTheme.light.copyWith(platform: () => scenario.platform),
              home: MediaQuery(
                data: MediaQueryData(devicePixelRatio: scenario.pixelRatio),
                child: Scaffold(child: HistoryScreen(controller: controller)),
              ),
            ),
          );
          await tester.pumpAndSettle();

          final image = find.byKey(
            const ValueKey<String>('history-card-image-proportional'),
          );
          final viewport = find
              .ancestor(of: image, matching: find.byType(LayoutBuilder))
              .first;
          final imageSize = tester.getSize(image);
          final availableWidth = tester.getSize(viewport).width;
          final imageContext = tester.element(image);
          final linePainter = TextPainter(
            text: TextSpan(
              text: 'Ag',
              style: DefaultTextStyle.of(imageContext).style,
            ),
            maxLines: 1,
            textDirection: Directionality.of(imageContext),
            textScaler: MediaQuery.textScalerOf(imageContext),
          )..layout();
          final maxHeight = linePainter.height * 8;

          expect(
            imageSize.width / imageSize.height,
            closeTo(dimensions.$1 / dimensions.$2, 0.05),
          );
          expect(imageSize.width, lessThanOrEqualTo(availableWidth));
          expect(imageSize.height, lessThanOrEqualTo(maxHeight));
          final expectedSize = applyBoxFit(
            BoxFit.scaleDown,
            Size(dimensions.$1.toDouble(), dimensions.$2.toDouble()),
            Size(availableWidth, maxHeight),
          ).destination;
          expect(imageSize.width, closeTo(expectedSize.width, 1));
          expect(imageSize.height, closeTo(expectedSize.height, 1));
          final expectedWidth = (availableWidth * scenario.pixelRatio)
              .ceil()
              .clamp(1, 2048);
          final expectedHeight = (maxHeight * scenario.pixelRatio).ceil().clamp(
            1,
            2048,
          );
          expect(repository.requestedImageEdges, [null]);
          expect(repository.requestedImageBounds.single?.width, expectedWidth);
          expect(
            repository.requestedImageBounds.single?.height,
            expectedHeight,
          );
          final provider = tester.widget<Image>(image).image as ResizeImage;
          expect(provider.width, lessThanOrEqualTo(expectedWidth));
          expect(provider.height, lessThanOrEqualTo(expectedHeight));
          expect(
            provider.width,
            lessThanOrEqualTo((imageSize.width * scenario.pixelRatio).ceil()),
          );
          expect(
            provider.height,
            lessThanOrEqualTo((imageSize.height * scenario.pixelRatio).ceil()),
          );
          expect(tester.takeException(), isNull);

          if (scenario.platform == TargetPlatform.macOS &&
              dimensions == (1200, 200)) {
            await tester.binding.setSurfaceSize(const Size(700, 900));
            await tester.pumpAndSettle();
            final resizedWidth = tester.getSize(viewport).width;
            final resizedImage = tester.getSize(image);
            expect(resizedImage.width, closeTo(resizedWidth, 0.01));
            expect(resizedImage.width / resizedImage.height, closeTo(6, 0.05));
            expect(repository.requestedImageBounds.length, 2);
            expect(
              repository.requestedImageBounds.last?.width,
              (resizedWidth * scenario.pixelRatio).ceil(),
            );
            expect(
              repository.requestedImageBounds.last?.height,
              expectedHeight,
            );
            expect(tester.takeException(), isNull);
          }
        },
      );
    }
  }

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
    expect(find.text('Observed app'), findsNothing);
    expect(
      find.byKey(const ValueKey<String>('history-detail-source-app')),
      findsNothing,
    );
    expect(repository.requestedSourceIconIds, isEmpty);

    final close = find.byKey(
      const ValueKey<String>('history-detail-inspector-close'),
    );
    expect(close, findsOneWidget);
    expect(find.bySemanticsLabel('Close clip details'), findsOneWidget);
    await tester.tap(close);
    await tester.pumpAndSettle();

    expect(
      find.byKey(const ValueKey<String>('history-detail-inspector')),
      findsNothing,
    );
    expect(tester.getSize(list).width, closeTo(1400 - AppSpacing.xxxl, 1));
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
              sourceAppIconId: 'app:editor',
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
      expect(find.text('Copy plain text'), findsNothing);
      expect(find.bySemanticsLabel('Copy options'), findsOneWidget);
      expect(_detailAction('Pin'), findsOneWidget);
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
        'Observed app',
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
      expect(tester.widget<SourceAppLabel>(sourceApp).name, 'Editor');
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
        tester.widget<SourceAppLabel>(sourceApp).style?.fontSize,
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
    expect(tester.widget<DeviceLabel>(device).name, 'Work Mac');
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
    expect(find.text('Copy plain text'), findsNothing);
    expect(find.bySemanticsLabel('Copy options'), findsOneWidget);
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
    final metadata = find.byKey(
      const ValueKey<String>('history-detail-metadata'),
    );
    expect(
      find.descendant(of: scrollContent, matching: metadata),
      findsOneWidget,
    );
    final actionsTop = tester.getTopLeft(actions).dy;
    await tester.drag(scrollContent, const Offset(0, -300));
    await tester.pump();
    expect(tester.getTopLeft(actions).dy, actionsTop);

    await tester.tap(close);
    await tester.pumpAndSettle();
    expect(drawer, findsNothing);
    expect(drawerVisibility, [isTrue, isFalse]);
  });

  testWidgets('keeps the mobile image fixed while its metadata table scrolls', (
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
            sourceAppIconId: 'app:editor',
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
    final scrollContent = find.byKey(
      const ValueKey<String>('history-detail-scroll-metadata'),
    );
    final imageViewport = find.byKey(
      const ValueKey<String>('history-detail-image-viewport'),
    );
    final actions = find.byKey(
      const ValueKey<String>('history-detail-actions'),
    );
    expect(drawer, findsOneWidget);
    expect(scrollContent, findsOneWidget);
    expect(imageViewport, findsOneWidget);
    expect(
      find.ancestor(of: imageViewport, matching: find.byType(Scrollable)),
      findsNothing,
    );
    final metadata = find.byKey(
      const ValueKey<String>('history-detail-metadata'),
    );
    expect(
      find.descendant(of: scrollContent, matching: metadata),
      findsOneWidget,
    );
    final table = tester.widget<Table>(metadata);
    final tableContext = tester.element(metadata);
    for (final row in table.rows!) {
      final border = row
          .buildDefaultTheme(tableContext)
          .border!
          .resolve(const <WidgetState>{})!;
      expect(border.bottom.color, Theme.of(tableContext).colorScheme.border);
      expect(border.bottom.width, 1);
    }
    final device = find.descendant(
      of: scrollContent,
      matching: find.byKey(const ValueKey<String>('history-detail-device')),
    );
    expect(device, findsOneWidget);
    expect(tester.widget<DeviceLabel>(device).name, 'Work Mac');
    expect(
      find.descendant(of: device, matching: find.byIcon(LucideIcons.laptop)),
      findsOneWidget,
    );
    final scrollable = find.descendant(
      of: scrollContent,
      matching: find.byType(Scrollable),
    );
    final scrollState = tester.state<ScrollableState>(scrollable.first);
    expect(scrollState.position.maxScrollExtent, greaterThan(0));
    expect(find.descendant(of: scrollContent, matching: actions), findsNothing);
    expect(
      tester.getTopLeft(imageViewport).dy,
      greaterThanOrEqualTo(tester.getTopLeft(fittedContent).dy),
    );
    expect(
      tester.getBottomLeft(imageViewport).dy,
      lessThanOrEqualTo(tester.getTopLeft(scrollContent).dy),
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
    final actionsRect = tester.getRect(actions);
    final imageRect = tester.getRect(imageViewport);
    final metadataTop = tester.getTopLeft(metadata).dy;
    await tester.drag(scrollContent, const Offset(0, -300));
    await tester.pumpAndSettle();
    expect(scrollState.position.pixels, greaterThan(0));
    expect(tester.getRect(imageViewport), imageRect);
    expect(tester.getTopLeft(metadata).dy, lessThan(metadataTop));
    expect(tester.getRect(actions), actionsRect);
    expect(_detailAction('Copy').hitTestable(), findsOneWidget);
    expect(_detailAction('Pin').hitTestable(), findsOneWidget);
    expect(_detailAction('Delete').hitTestable(), findsOneWidget);

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
    expect(tester.takeException(), isNull);
  });

  for (final size in [const Size(320, 568), const Size(700, 360)]) {
    testWidgets('anchors short mobile clip actions at the bottom at $size', (
      tester,
    ) async {
      await tester.binding.setSurfaceSize(size);
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final repository = _ScreenRepository()
        ..page = HistoryClipPage(
          items: [
            HistoryClip(
              id: 'short-mobile',
              contentType: 'text/plain',
              preview: 'Short mobile clip',
              createdAt: DateTime.utc(2026),
              pinned: false,
            ),
          ],
        );
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      await tester.pumpWidget(
        ShadcnApp(
          theme: AppTheme.light,
          home: Scaffold(child: HistoryScreen(controller: controller)),
        ),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.text('Short mobile clip'));
      await tester.pumpAndSettle();

      final card = find.byKey(
        const ValueKey<String>('history-detail-drawer-card'),
      );
      final actions = find.byKey(
        const ValueKey<String>('history-detail-actions'),
      );
      final kindIcon = find.byKey(
        const ValueKey<String>('history-detail-kind-icon'),
      );
      expect(
        tester.getSize(kindIcon),
        const Size.square(AppControlSize.compact),
      );
      final heading = find.descendant(
        of: find.byKey(const ValueKey<String>('history-detail-heading')),
        matching: find.text('Text'),
      );
      expect(
        tester.widget<Text>(heading).style?.fontSize,
        Theme.of(tester.element(heading)).typography.small.fontSize,
      );
      final cardBody = find.descendant(of: card, matching: find.byType(Column));
      expect(
        tester.getBottomLeft(cardBody.first).dy -
            tester.getBottomLeft(actions).dy,
        AppSpacing.xs,
      );
      for (final label in ['Copy', 'Pin', 'Delete']) {
        expect(_detailAction(label).hitTestable(), findsOneWidget);
      }
      expect(find.text('Copy plain text'), findsNothing);
      expect(
        find
            .byKey(const ValueKey<String>('history-copy-options'))
            .hitTestable(),
        findsOneWidget,
      );
      expect(tester.takeException(), isNull);
    });
  }

  testWidgets('renders backend facet labels without exposing their IDs', (
    tester,
  ) async {
    final repository = _ScreenRepository()
      ..availableFacets = const HistoryFacets(
        originDevices: [
          HistoryDeviceFacet(
            id: 'device-1',
            label: 'Work Mac',
            deviceClass: DeviceClass.laptop,
          ),
        ],
        sourceApps: [
          HistorySourceAppFacet(
            id: 'com.example.editor',
            label: 'Editor',
            iconId: 'app:com.example.editor',
          ),
        ],
      )
      ..sourceIcons['app:com.example.editor'] = HistorySourceAppIcon(
        base64Decode(
          'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNgYAAAAAMAASsJTYQAAAAASUVORK5CYII=',
        ),
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
    final workMac = find.byWidgetPredicate(
      (widget) => widget is DeviceLabel && widget.name == 'Work Mac',
    );
    expect(workMac, findsOneWidget);
    expect(find.byIcon(LucideIcons.laptop), findsOneWidget);
    expect(find.text('device-1'), findsNothing);
    expect(find.text('com.example.editor'), findsNothing);
    await tester.tap(workMac);
    await tester.pumpAndSettle();

    await tester.tap(find.bySemanticsLabel('All apps'));
    await tester.pump(const Duration(milliseconds: 200));
    await tester.pump(const Duration(milliseconds: 1));
    final editorLabel = find.byWidgetPredicate(
      (widget) => widget is SourceAppLabel && widget.name == 'Editor',
    );
    expect(editorLabel, findsOneWidget);
    final editorAvatar = tester.widget<Avatar>(
      find.descendant(of: editorLabel, matching: find.byType(Avatar)),
    );
    expect(editorAvatar.provider, isA<MemoryImage>());
    expect(repository.requestedSourceIconIds, ['app:com.example.editor']);
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
      ...find.byKey(const ValueKey<String>('history-import-files')).evaluate(),
      ...find.byWidgetPredicate((widget) => widget is Select).evaluate(),
    ];
    expect(searchButton, findsOneWidget);
    expect(
      find.byWidgetPredicate((widget) => widget is Select),
      findsNWidgets(5),
    );
    final controlHeight = tester.getSize(searchButton).height;
    expect(controlHeight, greaterThanOrEqualTo(AppControlSize.large));
    for (final control in controls) {
      final size = tester.getSize(find.byWidget(control.widget));
      expect(size, Size.square(controlHeight));
    }

    final search = tester.widget<Button>(searchButton);
    final searchContext = tester.element(searchButton);
    final searchDecoration =
        search.style.decoration(searchContext, const {}) as BoxDecoration;
    final firstSelectFinder = find
        .byWidgetPredicate((widget) => widget is Select)
        .first;
    final selectContext = tester.element(firstSelectFinder);
    final selectTheme = ComponentTheme.maybeOf<SelectTheme>(selectContext)!;
    final selectDecoration =
        selectTheme.decoration!(
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
    await tester.binding.setSurfaceSize(const Size(480, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final controller = HistoryController(_ScreenRepository());
    addTearDown(controller.dispose);
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

    final searchField = find.byType(TextField);
    expect(searchField, findsOneWidget);
    expect(tester.getSize(searchField).width, 160);
  });

  testWidgets(
    'preserves the search minimum at the labeled filter breakpoint',
    (tester) async {
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final controller = HistoryController(_ScreenRepository());
      addTearDown(controller.dispose);
      for (final textScale in [1.0, 1.5, 2.0]) {
        await tester.binding.setSurfaceSize(const Size(3200, 800));
        await tester.pumpWidget(
          ShadcnApp(
            theme: AppTheme.light,
            darkTheme: AppTheme.dark,
            themeMode: ThemeMode.light,
            builder: (context, child) => AppTheme.builder(
              context,
              MediaQuery(
                data: MediaQuery.of(
                  context,
                ).copyWith(textScaler: TextScaler.linear(textScale)),
                child: child!,
              ),
            ),
            home: HistoryScreen(controller: controller),
          ),
        );
        await tester.pumpAndSettle();

        final selects = find.byWidgetPredicate((widget) => widget is Select);
        expect(
          selects.evaluate().every(
            (element) => (element.widget as Select).expandIcon != null,
          ),
          isTrue,
        );
        final filterWidth = selects.evaluate().fold<double>(
          0,
          (sum, element) =>
              sum + tester.getSize(find.byWidget(element.widget)).width,
        );
        final breakpoint =
            160 +
            filterWidth +
            tester
                .getSize(
                  find.byKey(const ValueKey<String>('history-import-files')),
                )
                .width +
            (AppSpacing.sm * 6) +
            (AppSpacing.lg * 2);
        for (final width in [breakpoint + 1, breakpoint, breakpoint - 1]) {
          await tester.binding.setSurfaceSize(Size(width, 800));
          await tester.pumpAndSettle();
          expect(tester.takeException(), isNull);
          expect(find.byType(TextField), findsOneWidget);
          expect(
            tester.getSize(find.byType(TextField)).width,
            greaterThanOrEqualTo(160),
            reason: 'Window width $width, text scale $textScale',
          );
          final height = tester
              .getSize(
                find.byKey(const ValueKey<String>('history-import-files')),
              )
              .height;
          expect(
            tester.getSize(find.byType(TextField)).height,
            closeTo(height, 0.01),
          );
          for (final select in selects.evaluate()) {
            expect(
              tester.getSize(find.byWidget(select.widget)).height,
              closeTo(height, 0.01),
            );
          }
        }
      }
    },
    variant: TargetPlatformVariant({
      TargetPlatform.macOS,
      TargetPlatform.windows,
      TargetPlatform.android,
    }),
  );

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
            sourceAppIconId: 'app:editor',
          ),
        ],
      );
    final controller = HistoryController(repository);
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        darkTheme: AppTheme.dark,
        themeMode: ThemeMode.light,
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
    expect(meta.textSpan?.toPlainText(), isNot(contains('Text')));
    final kindIcon = find.descendant(
      of: metaFinder,
      matching: find.byIcon(LucideIcons.type),
    );
    expect(kindIcon, findsOneWidget);
    expect(tester.widget<Icon>(kindIcon).size, AppIconSize.xs);
    expect(meta.style?.fontSize, AppTypographySize.historyMetadata);
    expect(meta.style?.height, 1);
    expect(tester.getSize(metaFinder).height, 12);
    expect(
      tester.getSize(find.byType(Avatar)),
      const Size.square(AppIconSize.xs),
    );
    expect(
      find.descendant(
        of: find.byKey(const ValueKey<String>('history-clip-meta')),
        matching: find.byIcon(LucideIcons.pin),
      ),
      findsNothing,
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
            sourceApp: 'Editor',
            sourceAppIconId: 'app:editor',
            origin: 'Work Mac',
            originDeviceClass: DeviceClass.laptop,
          ),
        ],
      );
    final controller = HistoryController(repository);
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        darkTheme: AppTheme.dark,
        themeMode: ThemeMode.light,
        home: Scaffold(child: HistoryScreen(controller: controller)),
      ),
    );
    await tester.pumpAndSettle();

    final metaFinder = find.byKey(
      const ValueKey<String>('history-clip-meta-wide-meta'),
    );
    final meta = tester.widget<Text>(metaFinder);
    expect(meta.style?.fontSize, 12);
    expect(meta.style?.height, 1);
    expect(tester.getSize(metaFinder).height, 12);
    expect(
      tester.widget<SourceAppLabel>(find.byType(SourceAppLabel)).iconSize,
      AppIconSize.xs,
    );
    expect(
      tester.widget<DeviceLabel>(find.byType(DeviceLabel)).iconSize,
      AppIconSize.xs,
    );
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
    final originDevices = List<HistoryDeviceFacet>.generate(
      40,
      (index) => HistoryDeviceFacet(
        id: 'device-$index',
        label: 'Device $index',
        deviceClass: DeviceClass.unknown,
      ),
    );
    final sourceApps = List<HistorySourceAppFacet>.generate(
      40,
      (index) => HistorySourceAppFacet(id: 'app-$index', label: 'App $index'),
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
    await tester.tap(
      find.byWidgetPredicate(
        (widget) => widget is DeviceLabel && widget.name == 'Device 0',
      ),
    );
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
        final selectTheme = ComponentTheme.maybeOf<SelectTheme>(selectContext)!;
        final decoration =
            selectTheme.decoration!(
                  selectContext,
                  states,
                  const BoxDecoration(),
                )
                as BoxDecoration;
        expect(decoration.border?.top.style, BorderStyle.none);
      }
    }
  });

  for (final (platform, size) in [
    (TargetPlatform.macOS, const Size(1400, 900)),
    (TargetPlatform.windows, const Size(1400, 900)),
    (TargetPlatform.android, const Size(320, 568)),
    (TargetPlatform.android, const Size(700, 360)),
  ]) {
    testWidgets(
      'copies from the split button and its select on $platform at $size',
      (tester) async {
        await tester.binding.setSurfaceSize(size);
        addTearDown(() => tester.binding.setSurfaceSize(null));
        final repository = _ScreenRepository()
          ..page = HistoryClipPage(
            items: [
              HistoryClip(
                id: 'copy-clip',
                contentType: 'text/html',
                preview: 'Copy menu clip',
                createdAt: DateTime.utc(2026),
                pinned: false,
              ),
            ],
          );
        final controller = HistoryController(repository);
        addTearDown(controller.dispose);
        await tester.pumpWidget(
          ShadcnApp(
            theme: AppTheme.light.copyWith(platform: () => platform),
            builder: AppTheme.builder,
            home: Scaffold(child: HistoryScreen(controller: controller)),
          ),
        );
        await tester.pumpAndSettle();
        await tester.tap(find.text('Copy menu clip'));
        await tester.pumpAndSettle();

        final copy = _detailAction('Copy');
        final options = find.byKey(
          const ValueKey<String>('history-copy-options'),
        );
        expect(find.text('Copy plain text'), findsNothing);
        expect(tester.getRect(copy).right, tester.getRect(options).left);
        expect(tester.getSize(copy).height, tester.getSize(options).height);

        await tester.tap(copy);
        await tester.pumpAndSettle();
        expect(repository.copiedIds, ['copy-clip']);
        expect(repository.plainTextCopiedIds, isEmpty);
        await tester.pump(const Duration(seconds: 6));
        await tester.pumpAndSettle();

        final drawerCount = tester
            .widgetList(find.byType(DrawerWrapper))
            .length;
        await tester.tap(options);
        // The select's ContextAnchor polls every frame while the popup is open.
        await tester.pump();
        await tester.pump(AppMotion.standard);
        final popup = find.byType(SelectPopup<bool>);
        expect(popup, findsOneWidget);
        expect(
          OverlayConfiguration.maybeOf(tester.element(popup)),
          isA<PopoverConfiguration>(),
        );
        expect(find.byType(DrawerWrapper), findsNWidgets(drawerCount));
        final popupRect = tester.getRect(popup);
        expect(popupRect.left, greaterThanOrEqualTo(0));
        expect(popupRect.top, greaterThanOrEqualTo(0));
        expect(popupRect.right, lessThanOrEqualTo(size.width));
        expect(popupRect.bottom, lessThanOrEqualTo(size.height));
        expect(popupRect.height, lessThan(AppControlSize.large * 2));
        final plainTextIcon = find.descendant(
          of: popup,
          matching: find.byIcon(LucideIcons.alignLeft),
        );
        expect(
          tester.getRect(find.text('Copy plain text')).left -
              tester.getRect(plainTextIcon).right,
          AppSpacing.sm,
        );
        expect(repository.copiedIds, ['copy-clip']);
        await tester.tap(find.text('Copy plain text'));
        await tester.pumpAndSettle();
        expect(repository.plainTextCopiedIds, ['copy-clip']);
        expect(repository.copiedIds, ['copy-clip']);
        expect(popup, findsNothing);
        expect(tester.takeException(), isNull);
        await tester.pump(const Duration(seconds: 6));
        await tester.pumpAndSettle();
      },
    );
  }

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
    expect(
      find.byKey(const ValueKey<String>('history-copy-options')),
      findsNothing,
    );
    expect(_detailAction('Download'), findsOneWidget);
    await tester.tap(_detailAction('Download'));
    await tester.pumpAndSettle();

    expect(repository.savedFiles, [('remote-file', '/tmp/report.pdf')]);
    expect(downloader.presented, ['/tmp/report.pdf']);
    await tester.pump(const Duration(seconds: 6));
  });
}

Finder _detailAction(String label) {
  if (label == 'Pin') {
    return find.byWidgetPredicate(
      (widget) =>
          widget is Button &&
          widget.key is ValueKey<String> &&
          (widget.key as ValueKey<String>).value.startsWith('history-pin-'),
    );
  }
  return find.byKey(ValueKey<String>('history-detail-${label.toLowerCase()}'));
}

Future<void> _pumpHoverActions(WidgetTester tester) async {
  // Tooltip anchors keep a tracking ticker active, so settle cannot finish.
  await tester.pump();
  await tester.pump(const Duration(seconds: 1));
  await tester.pump(AppMotion.standard);
}

void _expectNoRadixIcons(WidgetTester tester) {
  final radixIcons = tester
      .widgetList<Icon>(find.byType(Icon))
      .where((icon) => icon.icon?.fontFamily == 'RadixIcons');
  expect(radixIcons, isEmpty);
}

class _ScreenRepository implements HistoryRepository {
  @override
  Future<void> importFile(HistoryImportFile file) async {}

  HistoryClipPage page = const HistoryClipPage(items: []);
  HistoryFacets availableFacets = const HistoryFacets();
  final List<(String, String)> savedFiles = [];
  final Map<String, HistorySourceAppIcon> sourceIcons = {};
  final List<String> requestedSourceIconIds = [];
  final List<String> copiedIds = [];
  final List<String> plainTextCopiedIds = [];
  final List<String> deletedIds = [];
  final List<(String, bool)> pinnedUpdates = [];
  bool failPinUpdate = false;
  HistoryImagePreview? availableImagePreview;
  bool fitImagePreviewBounds = false;
  final List<int?> requestedImageEdges = [];
  final List<HistoryImagePreviewBounds?> requestedImageBounds = [];
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
  Future<HistoryImagePreview?> imagePreview(
    String id, {
    int? maxEdge,
    HistoryImagePreviewBounds? bounds,
  }) async {
    requestedImageEdges.add(maxEdge);
    requestedImageBounds.add(bounds);
    final preview = availableImagePreview;
    if (!fitImagePreviewBounds || preview == null || bounds == null) {
      return preview;
    }
    final size = applyBoxFit(
      BoxFit.scaleDown,
      Size(preview.width.toDouble(), preview.height.toDouble()),
      Size(bounds.width.toDouble(), bounds.height.toDouble()),
    ).destination;
    final width = size.width.round().clamp(1, bounds.width);
    final height = size.height.round().clamp(1, bounds.height);
    return HistoryImagePreview(
      raster.encodePng(raster.Image(width: width, height: height)),
      width: width,
      height: height,
    );
  }

  @override
  Future<HistorySourceAppIcon?> sourceAppIcon(String id) async {
    requestedSourceIconIds.add(id);
    return sourceIcons[id];
  }

  @override
  Future<void> copy(String id) async => copiedIds.add(id);

  @override
  Future<void> copyPlainText(String id) async => plainTextCopiedIds.add(id);

  @override
  Future<void> saveFile(String id, String destinationPath) async {
    savedFiles.add((id, destinationPath));
  }

  @override
  Future<void> delete(String id) async => deletedIds.add(id);

  @override
  Future<void> deleteAll() async {}

  @override
  Future<void> reorderPinned(List<String> ids) async {}

  @override
  Future<void> setPinned(String id, bool pinned) async {
    if (failPinUpdate) throw StateError('Pin update failed');
    pinnedUpdates.add((id, pinned));
  }
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
