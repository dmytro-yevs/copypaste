import 'dart:ui' as ui;

import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(() async {
    final font = FontLoader('packages/shadcn_flutter/GeistSans');
    for (final face in ['Regular', 'Medium', 'SemiBold']) {
      font.addFont(
        rootBundle.load('packages/shadcn_flutter/lib/fonts/Geist-$face.otf'),
      );
    }
    await font.load();
  });

  testWidgets(
    'shared navigation and metadata keep glyphs inside the line box',
    (tester) async {
      late BuildContext context;
      await tester.pumpWidget(
        ShadcnApp(
          theme: AppTheme.dark,
          home: Builder(
            builder: (value) {
              context = value;
              return const SizedBox();
            },
          ),
        ),
      );
      await tester.runAsync(() async {
        for (final style in [
          AppTheme.navigationTextStyle(context),
          AppTheme.historyMetadataTextStyle(context),
          AppTheme.mobileNavigationLabelStyle,
          Theme.of(context).typography.textSmall,
        ]) {
          final painter = TextPainter(
            text: TextSpan(
              text: 'History Devices Settings CopyPaste gjpqy ЙЇ руф',
              style: style.copyWith(
                fontFamily: 'packages/shadcn_flutter/GeistSans',
                color: const Color(0xFFFFFFFF),
              ),
            ),
            textDirection: TextDirection.ltr,
          )..layout();
          addTearDown(painter.dispose);
          // Paint with a gutter so ink outside the line box remains observable.
          const gutter = 8.0;
          const density = 3.0;
          final recorder = ui.PictureRecorder();
          final canvas = Canvas(recorder)..scale(density);
          painter.paint(canvas, const Offset(gutter, gutter));
          final picture = recorder.endRecording();
          final image = await picture.toImage(
            ((painter.width + gutter * 2) * density).ceil(),
            ((painter.height + gutter * 2) * density).ceil(),
          );
          final pixels = (await image.toByteData())!;
          var outsideInk = 0;
          for (var y = 0; y < image.height; y++) {
            if (y >= gutter * density &&
                y < ((gutter + painter.height) * density).ceil()) {
              continue;
            }
            for (var x = 0; x < image.width; x++) {
              if (pixels.getUint8((y * image.width + x) * 4 + 3) > 0) {
                outsideInk++;
              }
            }
          }
          image.dispose();
          picture.dispose();
          expect(outsideInk, 0, reason: 'Glyphs must fit inside $style');
        }
      });
    },
  );
}
