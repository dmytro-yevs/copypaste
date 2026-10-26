import 'dart:io';

import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/app/theme/app_tokens.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  for (final dark in [false, true]) {
    for (final width in [360.0, 1200.0]) {
      for (final scale in [1.0, 2.0]) {
        testWidgets(
          'multi-select menu and chips at width $width, scale $scale, dark $dark',
          (tester) async {
            await tester.binding.setSurfaceSize(Size(width, 900));
            addTearDown(() => tester.binding.setSurfaceSize(null));
            var selected = <String>['English', 'Ukrainian'];
            final options = [
              'English',
              'Ukrainian',
              'German',
              'French',
              'Spanish',
              'Italian',
              'Portuguese',
              'Dutch',
              'Polish',
              'Japanese',
              'Chinese',
              'Korean',
            ];
            await tester.pumpWidget(
              ShadcnApp(
                theme: dark ? AppTheme.dark : AppTheme.light,
                builder: (context, child) => MediaQuery(
                  data: MediaQuery.of(context)
                      .copyWith(textScaler: TextScaler.linear(scale)),
                  child: Builder(
                    builder: (context) => AppTheme.builder(context, child),
                  ),
                ),
                home: Scaffold(
                  child: Padding(
                    padding: const EdgeInsets.all(AppSpacing.lg),
                    child: Align(
                      alignment: Alignment.topLeft,
                      child: SizedBox(
                        width: width < 600 ? width - AppSpacing.lg * 2 : 480,
                        child: StatefulBuilder(
                          builder: (context, setState) => Column(
                            mainAxisSize: MainAxisSize.min,
                            crossAxisAlignment: CrossAxisAlignment.stretch,
                            children: [
                              MultiSelect<String>(
                                key: const ValueKey('languages'),
                                value: selected,
                                onChanged: (values) => setState(
                                  () => selected = (values ?? const <String>[])
                                      .toList(),
                                ),
                                itemBuilder: (context, value) =>
                                    MultiSelectChip(
                                      value: value,
                                      style: AppTheme.selectChipStyle,
                                      child: Text(
                                        value,
                                        maxLines: 1,
                                        overflow: TextOverflow.ellipsis,
                                      ),
                                    ),
                                popup: SelectPopup<String>(
                                  items: SelectItemList(
                                    children: [
                                      for (final value in options)
                                        SelectItemButton<String>(
                                          value: value,
                                          child: Text(
                                            value,
                                            maxLines: 1,
                                            overflow: TextOverflow.ellipsis,
                                          ),
                                        ),
                                    ],
                                  ),
                                ).call,
                              ),
                              const Gap(AppSpacing.sm),
                              Button.primary(
                                key: const ValueKey('save'),
                                style: AppTheme.actionButtonStyle(
                                  const ButtonStyle.primary(),
                                ),
                                onPressed: () {},
                                child: const Text('Save'),
                              ),
                            ],
                          ),
                        ),
                      ),
                    ),
                  ),
                ),
              ),
            );
            await tester.pumpAndSettle();
            expect(find.byType(MultiSelectChip), findsNWidgets(2));
            final trigger = find.byKey(const ValueKey('languages'));
            await tester.tap(
              find.descendant(
                of: trigger,
                matching: find.byType(SelectExpandIcon),
              ),
            );
            await tester.pump();
            await tester.pump(const Duration(milliseconds: 300));
            final popup = find.byType(SelectPopup<String>);
            final popupRect = tester.getRect(popup);
            final triggerRect = tester.getRect(trigger);
            expect(popupRect.width, closeTo(triggerRect.width, 0.01));
            expect(popupRect.left, closeTo(triggerRect.left, 0.01));
            expect(popupRect.right, lessThanOrEqualTo(width));
            expect(
              popupRect.height,
              lessThanOrEqualTo(AppOverlaySize.selectMaxHeight),
            );
            for (final item in tester.widgetList<SelectItemButton<String>>(
              find.byType(SelectItemButton<String>),
            )) {
              final clickable = tester.widget<Clickable>(
                find
                    .descendant(
                      of: find.byWidget(item),
                      matching: find.byType(Clickable),
                    )
                    .first,
              );
              final decoration =
                  clickable.decoration!.resolve(const {}) as BoxDecoration;
              expect(
                decoration.border?.top.style ?? BorderStyle.none,
                BorderStyle.none,
              );
            }
            await tester.drag(popup, const Offset(0, -900));
            await tester.pump();
            await tester.pump(const Duration(milliseconds: 300));
            expect(find.text('Korean').hitTestable(), findsOneWidget);
            await tester.drag(popup, const Offset(0, 900));
            await tester.pump();
            await tester.pump(const Duration(milliseconds: 300));
            await tester.tap(find.text('English').last);
            await tester.pump();
            await tester.pump(const Duration(milliseconds: 300));
            expect(selected, ['Ukrainian']);
            await tester.sendKeyEvent(LogicalKeyboardKey.escape);
            await tester.pumpAndSettle();
            expect(popup, findsNothing);
            expect(
              tester.getSize(trigger).height,
              closeTo(
                tester.getSize(find.byKey(const ValueKey('save'))).height,
                0.01,
              ),
            );
            await tester.tap(find.byType(ChipButton));
            await tester.pumpAndSettle();
            expect(selected, isEmpty);
            expect(find.byType(MultiSelectChip), findsNothing);
            expect(tester.takeException(), isNull);
          },
          variant: TargetPlatformVariant({
            TargetPlatform.macOS,
            TargetPlatform.windows,
            TargetPlatform.android,
          }),
        );
      }
    }
  }

  testWidgets(
    'disabled multi-select cannot open or remove values',
    (tester) async {
      await tester.pumpWidget(
        ShadcnApp(
          theme: AppTheme.light,
          builder: AppTheme.builder,
          home: Scaffold(
            child: MultiSelect<String>(
              enabled: false,
              value: const ['English'],
              itemBuilder: (context, value) => MultiSelectChip(
                value: value,
                style: AppTheme.selectChipStyle,
                child: Text(
                  value,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              popup: const SelectPopup<String>(
                items: SelectItemList(
                  children: [
                    SelectItemButton(value: 'English', child: Text('English')),
                  ],
                ),
              ).call,
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.byType(MultiSelect<String>));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 300));
      expect(find.byType(SelectPopup<String>), findsNothing);
      expect(find.byType(ChipButton), findsNothing);
      expect(find.byType(MultiSelectChip), findsOneWidget);
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant({
      TargetPlatform.macOS,
      TargetPlatform.windows,
      TargetPlatform.android,
    }),
  );

  test(
    'selectors and button inheritance use the shared application contract',
    () {
      final offenders = <String>[];
      for (final entity in Directory('lib').listSync(recursive: true)) {
        if (entity is! File || !entity.path.endsWith('.dart')) continue;
        if (entity.path.replaceAll('\\', '/').contains('/app/theme/')) continue;
        final source = entity.readAsStringSync();
        if (RegExp(r'\b(?:SelectTheme|ButtonStyleOverride)\s*\(')
            .hasMatch(source)) {
          offenders.add(entity.path);
        }
      }
      expect(
        offenders,
        isEmpty,
        reason: 'Screens must use AppTheme select variants and style concrete action buttons.',
      );
    },
  );
}
