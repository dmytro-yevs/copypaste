import 'dart:ui' as ui;

import 'package:copypaste_flutter/app/theme/app_motion.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/app/theme/app_tooltip.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  testWidgets('an open tooltip schedules no frames', (tester) async {
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        builder: (context, child) => AppTheme.builder(context, child!),
        home: Scaffold(
          child: Center(
            child: AppTooltip(
              tooltip: (context) => const TooltipContainer(child: Text('Hint')),
              child: const Text('Anchor'),
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    final pointer = await tester.createGesture(
      kind: ui.PointerDeviceKind.mouse,
    );
    addTearDown(pointer.removePointer);
    await pointer.addPointer(location: Offset.zero);
    await pointer.moveTo(tester.getCenter(find.text('Anchor')));
    await tester.pump();
    expect(find.text('Hint'), findsNothing);

    await tester.pump(AppMotion.tooltipDelay);
    // A tooltip that polled its anchor on every frame would never settle.
    await tester.pumpAndSettle();
    expect(find.text('Hint'), findsOneWidget);
    expect(tester.binding.transientCallbackCount, 0);

    await pointer.moveTo(Offset.zero);
    await tester.pumpAndSettle();
    expect(find.text('Hint'), findsNothing);
    // The touch presentation stays open until a tap elsewhere.
  }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));
}
