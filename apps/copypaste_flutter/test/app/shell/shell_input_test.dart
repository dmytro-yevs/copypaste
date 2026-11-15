import 'package:copypaste_flutter/app/navigation/navigation.dart';
import 'package:copypaste_flutter/app/shell/shell.dart';
import 'package:copypaste_flutter/app/theme/app_overlays.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  late AppNavigationController navigation;

  late TextEditingController field;

  setUp(() {
    navigation = AppNavigationController();
    field = TextEditingController();
  });
  tearDown(() {
    navigation.dispose();
    field.dispose();
  });

  Future<void> pumpShell(WidgetTester tester, {WidgetBuilder? panel}) async {
    await tester.binding.setSurfaceSize(const Size(1200, 720));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        builder: AppTheme.builder,
        home: AppShell(
          controller: navigation,
          headerActions: {
            AppDestination.devices: [
              Builder(
                builder: (context) => Button.secondary(
                  key: const ValueKey('open-panel'),
                  onPressed: () => showOverlay<void>(
                    context,
                    AppOverlays.drawerConfiguration(context),
                    builder:
                        panel ??
                        (context) => ConstrainedBox(
                          constraints: AppOverlays.drawerContentConstraints(
                            context,
                          ),
                          child: const Scaffold(child: Text('Panel')),
                        ),
                  ),
                  child: const Text('Open panel'),
                ),
              ),
            ],
          },
          destinations: {
            AppDestination.history: TextField(
              key: const ValueKey('history-field'),
              controller: field,
            ),
            AppDestination.devices: const Text('Devices body'),
            AppDestination.settings: const Text('Settings body'),
          },
        ),
      ),
    );
    await tester.pump();
  }

  Future<void> pressDigit(WidgetTester tester, LogicalKeyboardKey digit) async {
    final modifier = defaultTargetPlatform == TargetPlatform.macOS
        ? LogicalKeyboardKey.metaLeft
        : LogicalKeyboardKey.controlLeft;
    await tester.sendKeyDownEvent(modifier);
    await tester.sendKeyEvent(digit);
    await tester.sendKeyUpEvent(modifier);
    await tester.pumpAndSettle();
  }

  Future<void> pushDetail(WidgetTester tester, String body) async {
    navigation
        .push<void>(
          tester.element(find.text(body)),
          builder: (context) => const Scaffold(child: Text('Detail route')),
        )
        .ignore();
    await tester.pumpAndSettle();
    expect(find.text('Detail route'), findsOneWidget);
  }

  Future<void> showDialog(WidgetTester tester) async {
    AppOverlays.showDialog<void>(
      tester.element(find.byType(AppShell)),
      builder: (context) => AppOverlays.alertDialog(
        icon: LucideIcons.info,
        title: const Text('Modal'),
        actions: [
          Button.primary(
            onPressed: () => Navigator.pop(context),
            child: const Text('Close modal'),
          ),
        ],
      ),
    ).ignore();
    await tester.pumpAndSettle();
    expect(find.text('Modal'), findsOneWidget);
  }

  testWidgets(
    'the command modifier with 1, 2 and 3 activates the destinations',
    (tester) async {
      await pumpShell(tester);
      // Editing text keeps the combination, except while an input method
      // composes and may use the same keys.
      await tester.tap(find.byKey(const ValueKey('history-field')));
      await tester.pump();
      field.value = const TextEditingValue(
        text: 'ni',
        selection: TextSelection.collapsed(offset: 2),
        composing: TextRange(start: 0, end: 2),
      );
      await pressDigit(tester, LogicalKeyboardKey.digit2);
      expect(navigation.selectedDestination, AppDestination.history);
      field.value = const TextEditingValue(
        text: 'ni',
        selection: TextSelection.collapsed(offset: 2),
      );

      await pressDigit(tester, LogicalKeyboardKey.digit2);
      expect(navigation.selectedDestination, AppDestination.devices);
      await pressDigit(tester, LogicalKeyboardKey.digit3);
      expect(navigation.selectedDestination, AppDestination.settings);
      await pressDigit(tester, LogicalKeyboardKey.digit1);
      expect(navigation.selectedDestination, AppDestination.history);

      // The current destination's own key returns it to its root.
      await pressDigit(tester, LogicalKeyboardKey.digit2);
      await pushDetail(tester, 'Devices body');
      await pressDigit(tester, LogicalKeyboardKey.digit2);
      expect(find.text('Detail route'), findsNothing);
      expect(navigation.selectedDestination, AppDestination.devices);

      // Each destination names its shortcut where it is chosen.
      final modifier = defaultTargetPlatform == TargetPlatform.macOS
          ? '⌘'
          : 'Ctrl+';
      for (final (index, destination) in appNavigationDestinations.indexed) {
        final tooltip = tester.widget<Tooltip>(
          find.ancestor(
            of: find.byKey(
              ValueKey('header-destination-${destination.destination.name}'),
            ),
            matching: find.byType(Tooltip),
          ),
        );
        final content =
            tooltip.tooltip(tester.element(find.byType(AppShell)))
                as TooltipContainer;
        expect(
          (content.child as Text).data,
          '${destination.label} · $modifier${index + 1}',
        );
      }

      await showDialog(tester);
      await pressDigit(tester, LogicalKeyboardKey.digit1);
      expect(navigation.selectedDestination, AppDestination.devices);
      await tester.tap(find.text('Close modal'));
      await tester.pumpAndSettle();
      await pressDigit(tester, LogicalKeyboardKey.digit1);
      expect(navigation.selectedDestination, AppDestination.history);
      expect(tester.takeException(), isNull);
    },
    variant: const TargetPlatformVariant({
      TargetPlatform.macOS,
      TargetPlatform.windows,
      TargetPlatform.linux,
    }),
  );

  testWidgets(
    'Tab starts at the first header control and a closed panel returns focus',
    (tester) async {
      await pumpShell(tester);
      bool focusedWithin(Finder finder) {
        final focused = FocusManager.instance.primaryFocus?.context;
        if (focused == null) return false;
        final target = tester.element(finder);
        var found = identical(focused, target);
        focused.visitAncestorElements((element) {
          found = found || identical(element, target);
          return !found;
        });
        return found;
      }

      // The shell receives keys before any control is focused.
      expect(FocusManager.instance.primaryFocus?.skipTraversal, isTrue);
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pump();
      expect(focusedWithin(find.byKey(const ValueKey('header-brand'))), isTrue);
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pump();
      expect(
        focusedWithin(find.byKey(const ValueKey('header-destination-history'))),
        isTrue,
      );
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pump();
      expect(
        focusedWithin(find.byKey(const ValueKey('header-destination-devices'))),
        isTrue,
      );
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(navigation.selectedDestination, AppDestination.devices);

      final opener = find.byKey(const ValueKey('open-panel'));
      for (var step = 0; step < 4 && !focusedWithin(opener); step++) {
        await tester.sendKeyEvent(LogicalKeyboardKey.tab);
        await tester.pump();
      }
      expect(focusedWithin(opener), isTrue);
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(find.text('Panel'), findsOneWidget);
      expect(focusedWithin(opener), isFalse);

      // Escape takes the shell's back order and closes the panel first.
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      expect(find.text('Panel'), findsNothing);
      expect(navigation.selectedDestination, AppDestination.devices);
      expect(focusedWithin(opener), isTrue);
      expect(tester.takeException(), isNull);
    },
    variant: const TargetPlatformVariant({
      TargetPlatform.macOS,
      TargetPlatform.windows,
    }),
  );

  testWidgets(
    'the logo and wordmark open History',
    (tester) async {
      await pumpShell(tester);
      navigation.selectDestination(AppDestination.devices);
      await tester.pump();
      expect(
        find.descendant(
          of: find.byKey(const ValueKey('header-brand')),
          matching: find.byKey(const ValueKey('header-navigation-logo')),
        ),
        findsOneWidget,
      );
      await tester.tap(find.byKey(const ValueKey('header-wordmark')));
      await tester.pumpAndSettle();
      expect(navigation.selectedDestination, AppDestination.history);
    },
    variant: const TargetPlatformVariant(<TargetPlatform>{
      TargetPlatform.macOS,
      TargetPlatform.windows,
    }),
  );

  testWidgets(
    'the mouse back button closes the panel, the nested page, then returns',
    (tester) async {
      final back = defaultTargetPlatform != TargetPlatform.macOS;
      await pumpShell(tester);
      navigation.selectDestination(AppDestination.devices);
      await tester.pump();
      await pushDetail(tester, 'Devices body');
      await tester.tap(find.byKey(const ValueKey('open-panel')));
      await tester.pumpAndSettle();
      expect(find.text('Panel'), findsOneWidget);

      final pointer = TestPointer(7, PointerDeviceKind.mouse);
      final position = tester.getCenter(find.byType(AppShell));
      Future<void> pressBack() async {
        await tester.sendEventToBinding(
          pointer.down(position, buttons: kBackMouseButton),
        );
        await tester.sendEventToBinding(pointer.up());
        await tester.pumpAndSettle();
      }

      await tester.sendEventToBinding(pointer.hover(position));
      // A back press that joins a held primary button belongs to a drag.
      await tester.sendEventToBinding(pointer.down(position));
      await tester.sendEventToBinding(
        pointer.move(
          position + const Offset(8, 0),
          buttons: kPrimaryMouseButton | kBackMouseButton,
        ),
      );
      await tester.sendEventToBinding(pointer.up());
      await tester.pumpAndSettle();
      expect(find.text('Panel'), findsOneWidget);

      await pressBack();
      expect(find.text('Panel'), back ? findsNothing : findsOneWidget);
      expect(find.text('Detail route'), findsOneWidget);
      await pressBack();
      expect(find.text('Detail route'), back ? findsNothing : findsOneWidget);
      expect(navigation.selectedDestination, AppDestination.devices);
      await pressBack();
      expect(
        navigation.selectedDestination,
        back ? AppDestination.history : AppDestination.devices,
      );
      await pressBack();
      expect(tester.takeException(), isNull);

      navigation.selectDestination(AppDestination.settings);
      await tester.pumpAndSettle();
      await showDialog(tester);
      await pressBack();
      expect(navigation.selectedDestination, AppDestination.settings);
      expect(find.text('Modal'), findsOneWidget);
    },
    variant: const TargetPlatformVariant({
      TargetPlatform.windows,
      TargetPlatform.linux,
      TargetPlatform.macOS,
    }),
  );

  testWidgets(
    'back actions close a drawer only through its own guard',
    (tester) async {
      var busy = true;
      await pumpShell(
        tester,
        panel: (context) => AppOverlays.drawerDismissal(
          onDismiss: () {
            if (!busy) closeDrawer(context);
          },
          child: const Text('Panel'),
        ),
      );
      navigation.selectDestination(AppDestination.devices);
      await tester.pump();
      await pushDetail(tester, 'Devices body');
      await tester.tap(find.byKey(const ValueKey('open-panel')));
      await tester.pumpAndSettle();

      Future<void> pressBack() async {
        final macOS = defaultTargetPlatform == TargetPlatform.macOS;
        final modifier = macOS
            ? LogicalKeyboardKey.metaLeft
            : LogicalKeyboardKey.altLeft;
        await tester.sendKeyDownEvent(modifier);
        await tester.sendKeyEvent(
          macOS ? LogicalKeyboardKey.bracketLeft : LogicalKeyboardKey.arrowLeft,
        );
        await tester.sendKeyUpEvent(modifier);
        await tester.pumpAndSettle();
      }

      // A refusing guard keeps the drawer, and the back action ends there.
      await pressBack();
      expect(find.text('Panel'), findsOneWidget);
      expect(find.text('Detail route'), findsOneWidget);

      busy = false;
      await pressBack();
      expect(find.text('Panel'), findsNothing);
      expect(find.text('Detail route'), findsOneWidget);
      expect(tester.takeException(), isNull);
    },
    variant: const TargetPlatformVariant({
      TargetPlatform.macOS,
      TargetPlatform.windows,
    }),
  );
}
