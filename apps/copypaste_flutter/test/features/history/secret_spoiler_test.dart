import 'dart:io';
import 'dart:ui' as ui;

import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/history/presentation/secret_spoiler.dart';
import 'package:copypaste_flutter/features/history/controller/history_spoiler_controller.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  for (final scale in [1.0, 2.0]) {
    for (final imageContent in [false, true]) {
      testWidgets(
        'mask keeps exact content geometry and shared visibility: scale=$scale image=$imageContent',
        (tester) async {
          await _loadFonts(tester);
          final semantics = tester.ensureSemantics();
          final controller = HistorySpoilerController(null);
          addTearDown(controller.dispose);
          final boundary = GlobalKey();
          final content = imageContent
              ? SizedBox(
                  width: 190,
                  height: 130,
                  child: ColoredBox(
                    color: Color(0xffff0000),
                    child: Semantics(label: 'SYNTHETIC SECRET FIXTURE'),
                  ),
                )
              : const Text(
                  'SYNTHETIC SECRET FIXTURE\nSecond line\nThird line',
                  style: TextStyle(
                    fontFamily: 'GeistSans',
                    package: 'shadcn_flutter',
                  ),
                );
          await tester.pumpWidget(
            ShadcnApp(
              theme: AppTheme.light,
              builder: (context, child) => AppTheme.builder(
                context,
                MediaQuery(
                  data: MediaQuery.of(context).copyWith(
                    textScaler: TextScaler.linear(scale),
                    disableAnimations: true,
                  ),
                  child: child!,
                ),
              ),
              home: Scaffold(
                child: RepaintBoundary(
                  key: boundary,
                  child: Center(
                    child: SizedBox(
                      width: 280,
                      child: Column(
                        mainAxisSize: MainAxisSize.min,
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          for (final index in [0, 1])
                            SecretSpoiler(
                              key: ValueKey('mask-$index'),
                              clipId: 'clip',
                              controller: controller,
                              child: content,
                            ),
                        ],
                      ),
                    ),
                  ),
                ),
              ),
            ),
          );
          await tester.pumpAndSettle();
          final size = tester.getSize(find.byKey(const ValueKey('mask-0')));
          expect(
            _semantics(tester),
            isNot(contains('SYNTHETIC SECRET FIXTURE')),
          );
          expect(
            find.bySemanticsLabel('Confidential content. Reveal spoiler'),
            findsNWidgets(2),
          );
          await tester.runAsync(
            () => _capture(boundary, 'spoiler-hidden-$scale-$imageContent.png'),
          );
          await tester.tap(find.byType(Button).first);
          await tester.pumpAndSettle();
          expect(controller.isRevealed('clip'), isTrue);
          expect(find.byType(Button), findsNothing);
          expect(_semantics(tester), contains('SYNTHETIC SECRET FIXTURE'));
          expect(tester.getSize(find.byKey(const ValueKey('mask-0'))), size);
          expect(tester.getSize(find.byKey(const ValueKey('mask-1'))), size);
          await tester.runAsync(
            () =>
                _capture(boundary, 'spoiler-revealed-$scale-$imageContent.png'),
          );
          tester.binding.handleAppLifecycleStateChanged(
            AppLifecycleState.inactive,
          );
          await tester.pump();
          expect(controller.isRevealed('clip'), isTrue);
          tester.binding.handleAppLifecycleStateChanged(
            AppLifecycleState.hidden,
          );
          await tester.pumpAndSettle();
          expect(controller.isRevealed('clip'), isFalse);
          // Hidden applications do not produce frames; verify the first frame
          // after resuming is already concealed.
          tester.binding.handleAppLifecycleStateChanged(
            AppLifecycleState.resumed,
          );
          await tester.pumpAndSettle();
          expect(
            _semantics(tester),
            isNot(contains('SYNTHETIC SECRET FIXTURE')),
          );
          expect(tester.getSize(find.byKey(const ValueKey('mask-0'))), size);
          tester.binding.handleAppLifecycleStateChanged(
            AppLifecycleState.resumed,
          );
          expect(tester.takeException(), isNull);
          await tester.pumpWidget(const SizedBox.shrink());
          semantics.dispose();
        },
      );
    }
  }
}

Future<void> _capture(GlobalKey boundary, String filename) async {
  final output = Platform.environment['COPYPASTE_SPOILER_CAPTURE_DIR'];
  if (output == null) return;
  final image =
      await (boundary.currentContext!.findRenderObject()!
              as RenderRepaintBoundary)
          .toImage(pixelRatio: 2);
  final bytes = await image.toByteData(format: ui.ImageByteFormat.png);
  await Directory(output).create(recursive: true);
  await File('$output/$filename').writeAsBytes(bytes!.buffer.asUint8List());
  image.dispose();
}

Future<void> _loadFonts(WidgetTester tester) async {
  await tester.runAsync(() async {
    final font = FontLoader('packages/shadcn_flutter/GeistSans');
    font.addFont(
      rootBundle.load('packages/shadcn_flutter/lib/fonts/Geist-Regular.otf'),
    );
    await font.load();
  });
}

String _semantics(WidgetTester tester) => tester
    .binding
    .renderViews
    .first
    .owner!
    .semanticsOwner!
    .rootSemanticsNode!
    .toStringDeep();
