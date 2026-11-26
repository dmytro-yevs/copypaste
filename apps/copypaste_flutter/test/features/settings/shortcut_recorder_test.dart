import 'package:copypaste_flutter/features/settings/view/shortcut_recorder.dart';
import 'package:copypaste_flutter/platform/desktop/global_shortcut.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  testWidgets('losing focus ends the recording', (tester) async {
    var cancellations = 0;
    await tester.pumpWidget(
      ShadcnApp(
        home: Center(
          child: ShortcutRecorder(
            shortcut: DesktopShortcut.defaultForPlatform(),
            onBeginRecording: () async => true,
            onCancelRecording: () async {
              cancellations += 1;
            },
            onRecorded: (_) {},
          ),
        ),
      ),
    );

    await tester.tap(find.byType(Button));
    await tester.pump();
    expect(find.text('Press shortcut…'), findsOneWidget);

    // The released registration must not outlive the recorder's focus.
    FocusManager.instance.primaryFocus!.unfocus();
    await tester.pump();
    await tester.pump();

    expect(cancellations, 1);
    expect(find.text('Press shortcut…'), findsNothing);
  });
}
