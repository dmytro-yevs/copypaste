import 'dart:async';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:copypaste_flutter/app/theme/app_motion.dart';
import 'package:copypaste_flutter/app/navigation/navigation.dart';
import 'package:copypaste_flutter/app/shell/app_shell.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/app/theme/app_tokens.dart';
import 'package:copypaste_flutter/features/history/controller/history_controller.dart';
import 'package:copypaste_flutter/features/history/models/history_models.dart';
import 'package:copypaste_flutter/features/history/repository/history_file_importer.dart';
import 'package:copypaste_flutter/features/history/repository/history_repository.dart';
import 'package:copypaste_flutter/features/history/view/history_screen.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/foundation.dart' show defaultTargetPlatform;
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:image/image.dart' as raster;
import 'package:shadcn_flutter/shadcn_flutter.dart';

const _platforms = TargetPlatformVariant({
  TargetPlatform.android,
  TargetPlatform.macOS,
  TargetPlatform.windows,
});

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async {
    final text = FontLoader('packages/shadcn_flutter/GeistSans');
    for (final face in ['Regular', 'Medium', 'SemiBold']) {
      text.addFont(
        rootBundle.load('packages/shadcn_flutter/lib/fonts/Geist-$face.otf'),
      );
    }
    await text.load();
    final icons = FontLoader('packages/shadcn_flutter/LucideIcons')
      ..addFont(
        rootBundle.load('packages/shadcn_flutter/lib/icons/LucideIcons.ttf'),
      );
    await icons.load();
  });
  testWidgets(
    'Ctrl Cmd and Shift select visual ranges and Delete confirms them',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(1400, 900));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final repository = _GestureRepository(count: 6);
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      await _mount(tester, controller);
      await tester.tap(_clip(0), kind: PointerDeviceKind.mouse);
      await _frames(tester);
      expect(controller.selectedId, 'clip-0');
      await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
      await tester.tap(_clip(2), kind: PointerDeviceKind.mouse);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
      await _frames(tester);
      expect(controller.bulkSelectedIds, {'clip-0', 'clip-2'});
      expect(controller.selectedId, isNull);
      await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
      await tester.tap(_clip(4), kind: PointerDeviceKind.mouse);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
      await _frames(tester);
      expect(controller.bulkSelectedIds, {'clip-2', 'clip-3', 'clip-4'});
      await tester.sendKeyDownEvent(LogicalKeyboardKey.metaLeft);
      await tester.tap(_clip(0), kind: PointerDeviceKind.mouse);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.metaLeft);
      await _frames(tester);
      expect(controller.bulkSelectedIds, {
        'clip-0',
        'clip-2',
        'clip-3',
        'clip-4',
      });
      await tester.sendKeyEvent(LogicalKeyboardKey.delete);
      await _frames(tester);
      expect(find.text('Delete 4 selected clips?'), findsOneWidget);
      expect(repository.deletedIds, isEmpty);
      await tester.tap(find.widgetWithText(Button, 'Cancel'));
      await _frames(tester);
      expect(controller.bulkSelectedIds, hasLength(4));
      await tester.sendKeyEvent(LogicalKeyboardKey.delete);
      await _frames(tester);
      await tester.tap(find.widgetWithText(Button, 'Delete'));
      await _frames(tester);
      expect(repository.deletedIds.toSet(), {
        'clip-0',
        'clip-2',
        'clip-3',
        'clip-4',
      });
      expect(controller.items.map((clip) => clip.id).toSet(), {
        'clip-1',
        'clip-5',
      });
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    },
    variant: _platforms,
  );

  testWidgets(
    'held touch and mouse select in both directions and survive wheel scrolling',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(700, 750));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      for (final kind in [PointerDeviceKind.touch, PointerDeviceKind.mouse]) {
        final repository = _GestureRepository(count: 30);
        final controller = HistoryController(repository);
        await _mount(tester, controller);
        final gesture = await tester.startGesture(
          tester.getCenter(_clip(0)),
          kind: kind,
        );
        await tester.pump(kLongPressTimeout + kPressTimeout);
        await tester.pump();
        expect(controller.bulkSelectedIds, {'clip-0'});
        expect(controller.isBulkDragSelecting, isTrue);
        await gesture.moveTo(tester.getCenter(_clip(3)));
        await _frames(tester);
        expect(controller.bulkSelectedIds, {
          'clip-0',
          'clip-1',
          'clip-2',
          'clip-3',
        });
        await gesture.moveTo(tester.getCenter(_clip(1)));
        await _frames(tester);
        expect(controller.bulkSelectedIds, {'clip-0', 'clip-1'});
        if (kind == PointerDeviceKind.mouse) {
          await tester.sendEventToBinding(
            PointerScrollEvent(
              position: tester.getCenter(_clip(1)),
              scrollDelta: const Offset(0, 170),
              kind: PointerDeviceKind.mouse,
            ),
          );
          await _frames(tester);
          expect(controller.bulkSelectedIds.length, greaterThan(2));
        }
        await gesture.up();
        await _frames(tester);
        expect(controller.isBulkDragSelecting, isFalse);
        final selected = Set<String>.of(controller.bulkSelectedIds);
        final scrollable = tester.state<ScrollableState>(
          find.byType(Scrollable).first,
        );
        final pixels = scrollable.position.pixels;
        await tester.pump(const Duration(milliseconds: 500));
        expect(scrollable.position.pixels, pixels);
        expect(controller.bulkSelectedIds, selected);
        await tester.drag(find.byType(ListView), const Offset(0, -170));
        await _frames(tester);
        expect(controller.bulkSelectedIds, selected);
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox.shrink());
        controller.dispose();
      }
    },
    variant: _platforms,
  );

  testWidgets(
    'holding at the viewport edge auto-scrolls and selects through new pages',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(320, 568));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      for (final kind in [PointerDeviceKind.touch, PointerDeviceKind.mouse]) {
        final repository = _GestureRepository(count: 40);
        final controller = HistoryController(repository, pageSize: 6);
        await _mount(tester, controller, shell: true);
        final gesture = await tester.startGesture(
          tester.getCenter(_clip(0)),
          kind: kind,
        );
        await tester.pump(kLongPressTimeout + kPressTimeout);
        await tester.pump();
        final viewport = tester.getRect(find.byType(ListView));
        await gesture.moveTo(
          Offset(
            viewport.center.dx,
            tester
                    .getRect(
                      find.byKey(const ValueKey('mobile-navigation-dock')),
                    )
                    .top -
                AppSpacing.xs,
          ),
        );
        for (var frame = 0; frame < 24; frame++) {
          await tester.pump(const Duration(milliseconds: 100));
        }
        expect(controller.items.length, greaterThan(6));
        expect(controller.bulkSelectedIds.length, greaterThan(6));
        expect(controller.bulkSelectedIds, contains('clip-0'));
        expect(repository.cursors, contains('6'));
        // The original row has left the viewport; the same held pointer still selects.
        expect(_clip(0).hitTestable(), findsNothing);
        await gesture.cancel();
        await _frames(tester);
        expect(controller.isBulkDragSelecting, isFalse);
        final selected = Set<String>.of(controller.bulkSelectedIds);
        await tester.pump(const Duration(milliseconds: 500));
        expect(controller.bulkSelectedIds, selected);
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox.shrink());
        controller.dispose();
      }
    },
    variant: _platforms,
  );

  testWidgets(
    'bulk image rows contain their previews and actions align right in the mobile shell',
    (tester) async {
      addTearDown(() => tester.binding.setSurfaceSize(null));
      for (final dark in [false, true]) {
        for (final scale in [1.0, 2.0]) {
          await tester.binding.setSurfaceSize(const Size(320, 700));
          final repository = _GestureRepository(count: 3, image: true);
          final controller = HistoryController(repository);
          final boundary = GlobalKey();
          await _mount(
            tester,
            controller,
            scale: scale,
            boundary: boundary,
            dark: dark,
            shell: true,
          );
          await tester.tap(find.byKey(const ValueKey('history-select-clips')));
          await _frames(tester);
          final visible = tester
              .getRect(_clip(0))
              .intersect(tester.getRect(find.byType(ListView)));
          expect(visible.isEmpty, isFalse);
          await tester.tapAt(visible.center);
          await _frames(tester);
          expect(controller.bulkSelectedIds, {'clip-0'});
          final imageFinder = find.byKey(
            const ValueKey('history-card-image-clip-0'),
          );
          await tester.runAsync(
            () => precacheImage(
              tester.widget<Image>(imageFinder).image,
              tester.element(imageFinder),
            ),
          );
          await _frames(tester);
          expect(
            tester
                .widget<RawImage>(
                  find.descendant(
                    of: imageFinder,
                    matching: find.byType(RawImage),
                  ),
                )
                .image,
            isNotNull,
          );
          final image = tester.getRect(
            find.byKey(const ValueKey('history-card-image-clip-0')),
          );
          final row = tester.getRect(_clip(0));
          expect(image.width, greaterThan(0));
          expect(image.height, greaterThan(0));
          expect(image.left, greaterThanOrEqualTo(row.left));
          expect(image.right, lessThanOrEqualTo(row.right));
          expect(image.top, greaterThanOrEqualTo(row.top));
          expect(image.bottom, lessThanOrEqualTo(row.bottom));
          final actions = [
            for (final id in ['close', 'pin', 'unpin', 'delete'])
              tester.getRect(find.byKey(ValueKey('history-bulk-$id'))),
          ];
          expect(actions.map((rect) => rect.size).toSet(), hasLength(1));
          for (final top in actions.map((rect) => rect.top).toSet()) {
            final rights = actions
                .where((rect) => rect.top == top)
                .map((rect) => rect.right);
            expect(
              rights.reduce((left, right) => left > right ? left : right),
              closeTo(320 - AppSpacing.lg, 0.01),
            );
          }
          expect(find.text('1 selected'), findsOneWidget);
          expect(tester.takeException(), isNull);
          await tester.runAsync(
            () => _capture(
              boundary,
              'bulk-${defaultTargetPlatform.name}-${dark ? "dark" : "light"}-$scale.png',
            ),
          );
          final scrollable = tester.state<ScrollableState>(
            find
                .descendant(
                  of: find.byType(ListView),
                  matching: find.byType(Scrollable),
                )
                .first,
          );
          final before = scrollable.position.pixels;
          if (_clip(1).evaluate().isEmpty) {
            await tester.scrollUntilVisible(
              _clip(1),
              AppControlSize.touch * 3,
              scrollable: find
                  .descendant(
                    of: find.byType(ListView),
                    matching: find.byType(Scrollable),
                  )
                  .first,
            );
            await _frames(tester);
          }
          final next = tester.getRect(_clip(1));
          expect(
            next.top + scrollable.position.pixels - before,
            greaterThanOrEqualTo(row.bottom),
          );
          expect(tester.takeException(), isNull);
          await tester.pumpWidget(const SizedBox.shrink());
          controller.dispose();
        }
      }
    },
    variant: _platforms,
  );
}

Finder _clip(int index) => find.byKey(ValueKey('history-clip-clip-$index'));

Future<void> _frames(WidgetTester tester) async {
  await tester.pump();
  await tester.pump(AppMotion.standard);
  await tester.pump();
}

Future<void> _mount(
  WidgetTester tester,
  HistoryController controller, {
  double scale = 1,
  GlobalKey? boundary,
  bool dark = false,
  bool shell = false,
}) async {
  Widget home = Scaffold(child: HistoryScreen(controller: controller));
  if (shell) {
    final navigation = AppNavigationController();
    addTearDown(navigation.dispose);
    home = AppShell(
      controller: navigation,
      destinations: {
        AppDestination.history: HistoryScreen(controller: controller),
        AppDestination.devices: const SizedBox.expand(),
        AppDestination.settings: const SizedBox.expand(),
      },
    );
  }
  await tester.pumpWidget(
    ShadcnApp(
      theme: _readableTheme(AppTheme.light),
      darkTheme: _readableTheme(AppTheme.dark),
      themeMode: dark ? ThemeMode.dark : ThemeMode.light,
      builder: (context, child) => AppTheme.builder(
        context,
        MediaQuery(
          data: MediaQuery.of(
            context,
          ).copyWith(textScaler: TextScaler.linear(scale)),
          child: child!,
        ),
      ),
      home: RepaintBoundary(key: boundary, child: home),
    ),
  );
  await _frames(tester);
  await tester.pump(const Duration(milliseconds: 100));
}

// Widget tests use the box-shaped Ahem font by default. The package's bundled
// font gives the captured layout real glyphs while retaining all AppTheme sizes.
ThemeData _readableTheme(ThemeData theme) => theme.copyWith(
  typography: () => theme.typography.copyWith(
    sans: () => theme.typography.sans.copyWith(
      fontFamily: 'packages/shadcn_flutter/GeistSans',
    ),
  ),
);

Future<void> _capture(GlobalKey boundary, String name) async {
  final output = Platform.environment['COPYPASTE_BULK_CAPTURE_DIR'];
  if (output == null) return;
  final image =
      await (boundary.currentContext!.findRenderObject()!
              as RenderRepaintBoundary)
          .toImage(pixelRatio: 2);
  final data = await image.toByteData(format: ui.ImageByteFormat.png);
  await Directory(output).create(recursive: true);
  await File('$output/$name').writeAsBytes(data!.buffer.asUint8List());
  image.dispose();
}

class _GestureRepository implements HistoryRepository {
  _GestureRepository({required int count, bool image = false}) {
    clips.addAll([
      for (var index = 0; index < count; index++)
        HistoryClip(
          id: 'clip-$index',
          contentType: image && index == 0 ? 'image/png' : 'text/plain',
          kind: image && index == 0
              ? HistoryClipKind.image
              : HistoryClipKind.text,
          preview: 'Synthetic clip $index',
          createdAt: DateTime.utc(2026, 10, 9, 12, 0, 50 - index),
          pinned: index == 0,
        ),
    ]);
    if (image) {
      final bitmap = raster.Image(width: 720, height: 1600);
      raster.fill(bitmap, color: raster.ColorRgb8(45, 95, 170));
      preview = HistoryImagePreview(
        raster.encodePng(bitmap),
        width: bitmap.width,
        height: bitmap.height,
      );
    }
  }
  final clips = <HistoryClip>[];
  final cursors = <String?>[];
  final deletedIds = <String>[];
  HistoryImagePreview? preview;
  @override
  Stream<HistoryRuntimeEvent> watch() => const Stream.empty();
  @override
  Future<HistoryFacets> facets() async => const HistoryFacets();
  @override
  Future<HistoryClipPage> query({
    required HistoryQuery query,
    required int limit,
    String? cursor,
  }) async {
    cursors.add(cursor);
    final start = int.parse(cursor ?? '0');
    final page = clips.skip(start).take(limit).toList();
    return HistoryClipPage(
      items: page,
      nextCursor: start + page.length < clips.length
          ? '${start + page.length}'
          : null,
    );
  }

  @override
  Future<HistoryClip> get(String id) async =>
      clips.firstWhere((clip) => clip.id == id);
  @override
  Future<HistoryImagePreview?> imagePreview(
    String id, {
    int? maxEdge,
    HistoryImagePreviewBounds? bounds,
  }) async => preview;
  @override
  Future<HistorySourceAppIcon?> sourceAppIcon(String id) async => null;
  @override
  Future<void> delete(String id) async {
    deletedIds.add(id);
    clips.removeWhere((clip) => clip.id == id);
  }

  @override
  Future<void> setPinned(String id, bool pinned) async {
    final index = clips.indexWhere((clip) => clip.id == id);
    clips[index] = clips[index].copyWith(pinned: pinned);
  }

  @override
  Future<void> copy(String id) async {}
  @override
  Future<void> copyPlainText(String id) async {}
  @override
  Future<void> saveFile(String id, String destinationPath) async {}
  @override
  Future<void> importFile(HistoryImportFile file) async {}
  @override
  Future<void> deleteAll() async {}
  @override
  Future<void> reorderPinned(List<String> ids) async {}
}
