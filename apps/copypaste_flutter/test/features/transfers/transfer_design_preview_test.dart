import 'dart:io';
import 'dart:ui' as ui;

import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/app/theme/app_tokens.dart';
import 'package:copypaste_flutter/transfer_design_preview.dart';
import 'package:copypaste_flutter/features/transfers/file_operation_tile.dart';
import 'package:copypaste_flutter/features/transfers/file_operations_controller.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  testWidgets('renders three compact concepts and removes one demo record', (
    tester,
  ) async {
    await tester.runAsync(() async {
      for (final font in [
        ('GeistSans', 'lib/fonts/Geist-Regular.otf'),
        ('LucideIcons', 'lib/icons/LucideIcons.ttf'),
      ]) {
        await (FontLoader('packages/shadcn_flutter/${font.$1}')
              ..addFont(rootBundle.load('packages/shadcn_flutter/${font.$2}')))
            .load();
      }
    });
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final output = Platform.environment['COPYPASTE_EVIDENCE_DIR'];
    Future<void> capture(GlobalKey key, String name) async {
      if (output == null) return;
      await tester.runAsync(() async {
        final image =
            await (key.currentContext!.findRenderObject()!
                    as RenderRepaintBoundary)
                .toImage(pixelRatio: 2);
        final bytes = await image.toByteData(format: ui.ImageByteFormat.png);
        await File('$output/$name.png')
            .writeAsBytes(bytes!.buffer.asUint8List());
        image.dispose();
      });
    }

    for (var concept = 0; concept < 3; concept++) {
      await tester.binding.setSurfaceSize(const Size(480, 520));
      final boundary = GlobalKey();
      await tester.pumpWidget(
        RepaintBoundary(
          key: boundary,
          child: TransferDesignPreviewApp(
            key: ValueKey(concept),
            concept: concept,
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      final active = find.byKey(const ValueKey('record-active'));
      expect(
        tester.getSize(active).height,
        lessThanOrEqualTo(concept == 2 ? 120 : 144),
      );
      expect(find.text('8.4 MB/s'), findsOneWidget);
      await capture(boundary, 'flutter-concept-${concept + 1}');
      final completed = find.byKey(const ValueKey('record-completed'));
      await tester.tap(
        find.descendant(
          of: completed,
          matching: find.byIcon(LucideIcons.trash2),
        ),
      );
      await tester.pumpAndSettle();
      expect(completed, findsNothing);
      expect(active, findsOneWidget);
    }
    await tester.binding.setSurfaceSize(const Size(1442, 520));
    final boundary = GlobalKey();
    await tester.pumpWidget(
      RepaintBoundary(key: boundary, child: const TransferDesignPreviewApp()),
    );
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
    await capture(boundary, 'flutter-concepts-comparison');
  });

  testWidgets(
    'selected tile preserves compact actions at mobile text scaling',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(360, 520));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      var cancellations = 0;
      var opened = 0;
      var removals = 0;
      final active = FileOperation(
        id: 'active',
        itemId: 'active',
        name: 'MobLand.2025.S01E01.Stick.or.Twist.mkv',
        kind: FileOperationKind.receive,
        phase: 'running',
        sourceDevice: 'CPH2305',
        destinationDevice: 'MacBook Air',
        written: (22.9 * 1024 * 1024).round(),
        total: (1143.3 * 1024 * 1024).round(),
        startedAt: DateTime(2026, 10, 21, 22, 13),
      );
      final completed = FileOperation(
        id: 'completed',
        itemId: 'completed',
        name: 'MobLand.2025.S01E03.Plan.B.mkv',
        kind: FileOperationKind.receive,
        phase: 'completed',
        sourceDevice: 'CPH2305',
        destinationDevice: 'This device',
        written: (567.5 * 1024 * 1024).round(),
        total: (567.5 * 1024 * 1024).round(),
        startedAt: DateTime.fromMillisecondsSinceEpoch(0),
        localPath: '/demo/MobLand.2025.S01E03.Plan.B.mkv',
      );

      await tester.pumpWidget(
        ShadcnApp(
          theme: AppTheme.dark,
          home: MediaQuery(
            data: const MediaQueryData(textScaler: TextScaler.linear(1.5)),
            child: Scaffold(
              child: SingleChildScrollView(
                padding: const EdgeInsets.all(AppSpacing.lg),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    SizedBox(
                      width: 328,
                      child: FileOperationTile(
                        key: const ValueKey('active-tile'),
                        operation: active,
                        speedLabel: '8.4 MB/s',
                        sourceDeviceIcon: LucideIcons.smartphone,
                        destinationDeviceIcon: LucideIcons.laptop,
                        onCancel: () => cancellations++,
                      ),
                    ),
                    const Gap(AppSpacing.lg),
                    SizedBox(
                      width: 328,
                      child: FileOperationTile(
                        key: const ValueKey('completed-tile'),
                        operation: completed,
                        sourceDeviceIcon: LucideIcons.smartphone,
                        destinationDeviceIcon: LucideIcons.laptop,
                        onOpenFile: () => opened++,
                        onRemove: () => removals++,
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(tester.takeException(), isNull);
      void press(Finder scope, IconData icon) {
        final button = find.ancestor(
          of: find.descendant(of: scope, matching: find.byIcon(icon)),
          matching: find.byType(Button),
        );
        tester.widget<Button>(button).onPressed!();
      }

      press(find.byKey(const ValueKey('active-tile')), LucideIcons.x);
      press(
        find.byKey(const ValueKey('completed-tile')),
        LucideIcons.folderOpen,
      );
      press(find.byKey(const ValueKey('completed-tile')), LucideIcons.trash2);

      expect(cancellations, 1);
      expect(opened, 1);
      expect(removals, 1);
    },
  );
}
