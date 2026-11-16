import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/history/repository/clip_edit_preferences.dart';
import 'package:copypaste_flutter/features/history/repository/clip_edit_repository.dart';
import 'package:copypaste_flutter/features/history/view/clip_edit_drawer.dart';
import 'package:copypaste_flutter/features/history/view/history_selectable_text.dart';
import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

class _UnusedClipEdits implements ClipEditRepository {
  @override
  Future<String> createManualClip(String text) => throw UnimplementedError();

  @override
  Future<String> saveClipEdit(
    String id,
    String text, {
    required bool replaceOriginal,
  }) => throw UnimplementedError();
}

Future<void> _pumpDrawer(WidgetTester tester, String text) async {
  await tester.binding.setSurfaceSize(const Size(1200, 800));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(
    ShadcnApp(
      theme: AppTheme.light,
      builder: AppTheme.builder,
      localizationsDelegates: AppLocalization.delegates,
      supportedLocales: AppLocalization.supportedLocales,
      home: Scaffold(
        child: ClipEditDrawer(
          clips: _UnusedClipEdits(),
          preferences: MemoryClipEditPreferences(),
          clip: ClipEditTarget(id: 'clip', text: text),
        ),
      ),
    ),
  );
  await tester.pump();
}

bool _canSave(WidgetTester tester) =>
    tester
        .widget<Button>(find.byKey(const ValueKey('clip-edit-save')))
        .onPressed !=
    null;

void main() {
  testWidgets('a clip over the edit limit is shown sliced, not in a field', (
    tester,
  ) async {
    await _pumpDrawer(
      tester,
      'line\n' * (ClipEditTarget.maxEditableBytes ~/ 5 + 1),
    );

    expect(find.byType(HistorySelectableText), findsOne);
    expect(find.byType(TextArea), findsNothing);
    // Any TextEditingController holding the body would sit in an EditableText.
    expect(find.byType(EditableText), findsNothing);
    expect(find.text(AppStrings.current.snippetsClipTooLarge), findsOne);
  });

  testWidgets('Save follows edits and turns off when the text is reverted', (
    tester,
  ) async {
    await _pumpDrawer(tester, 'original');
    final field = find.byKey(const ValueKey('clip-edit-text'));
    expect(_canSave(tester), isFalse);

    await tester.enterText(field, 'changed');
    await tester.pump();
    expect(_canSave(tester), isTrue);

    await tester.enterText(field, 'original');
    await tester.pump();
    expect(_canSave(tester), isFalse);
  });
}
