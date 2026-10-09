import 'dart:io';
import 'dart:ui' as ui;

import 'package:copypaste_flutter/app/theme/app_motion.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/history/presentation/secret_spoiler.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  testWidgets(
    'spoiler never builds plaintext before explicit reveal and hides on inactivity',
    (tester) async {
      await _loadFonts(tester);
      final semantics = tester.ensureSemantics();
      try {
        var reads = 0;
        final boundary = GlobalKey();
        await tester.pumpWidget(
          ShadcnApp(
            theme: AppTheme.light,
            builder: AppTheme.builder,
            home: RepaintBoundary(
              key: boundary,
              child: Scaffold(
                child: Center(
                  child: SizedBox(
                    width: 280,
                    child: SecretSpoiler(
                      reveal: (_) {
                        reads++;
                        return const Text(
                          'SYNTHETIC SECRET FIXTURE',
                          style: TextStyle(
                            fontFamily: 'GeistSans',
                            package: 'shadcn_flutter',
                          ),
                        );
                      },
                    ),
                  ),
                ),
              ),
            ),
          ),
        );
        await tester.pump();
        expect(reads, 0);
        expect(find.text('SYNTHETIC SECRET FIXTURE'), findsNothing);
        expect(
          find.bySemanticsLabel('Confidential content. Reveal spoiler'),
          findsOneWidget,
        );
        await tester.runAsync(() => _capture(boundary, 'spoiler-hidden.png'));
        await tester.tap(find.byType(Button));
        await tester.pump();
        await tester.pump(AppMotion.spoilerReveal);
        expect(reads, 1);
        expect(find.text('SYNTHETIC SECRET FIXTURE'), findsOneWidget);
        await tester.runAsync(() => _capture(boundary, 'spoiler-revealed.png'));
        tester.binding.handleAppLifecycleStateChanged(
          AppLifecycleState.inactive,
        );
        await tester.pump();
        expect(find.text('SYNTHETIC SECRET FIXTURE'), findsNothing);
        expect(reads, 1);
        tester.binding.handleAppLifecycleStateChanged(
          AppLifecycleState.resumed,
        );
        await tester.pumpWidget(const SizedBox.shrink());
      } finally {
        semantics.dispose();
      }
    },
  );

  testWidgets(
    'spoiler remains usable at 320px and 200 percent text with reduced motion',
    (tester) async {
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.binding.setSurfaceSize(const Size(320, 640));
      await tester.pumpWidget(
        ShadcnApp(
          theme: AppTheme.light,
          builder: (context, child) => AppTheme.builder(
            context,
            MediaQuery(
              data: MediaQuery.of(context).copyWith(
                disableAnimations: true,
                textScaler: TextScaler.linear(2),
              ),
              child: child!,
            ),
          ),
          home: Scaffold(
            child: SecretSpoiler(reveal: (_) => const Text('Synthetic secret')),
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(find.text('Synthetic secret'), findsNothing);
      await tester.tap(find.byType(Button));
      await tester.pumpAndSettle();
      expect(find.text('Synthetic secret'), findsOneWidget);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    },
  );
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
