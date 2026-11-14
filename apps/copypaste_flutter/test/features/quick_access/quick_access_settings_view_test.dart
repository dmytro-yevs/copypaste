import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/quick_access/controller/quick_access_controller.dart';
import 'package:copypaste_flutter/features/quick_access/view/quick_access_settings_section.dart';
import 'package:copypaste_flutter/features/quick_access/view/quick_access_settings_view.dart';
import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:copypaste_flutter/platform/android/android_quick_access_gateway.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

class _FakeGateway implements AndroidQuickAccessGateway {
  _FakeGateway(this.current);

  AndroidQuickAccessState current;
  final tileRequests = <AndroidQuickAccessTile>[];
  AndroidTileRequestResult tileAnswer = AndroidTileRequestResult.notAdded;

  @override
  Future<AndroidQuickAccessState> state() async => current;

  @override
  Future<bool> openKeyboardSettings() async => true;

  @override
  Future<bool> showKeyboardPicker() async => true;

  @override
  Future<AndroidTileRequestResult> requestAddTile(
    AndroidQuickAccessTile tile,
  ) async {
    tileRequests.add(tile);
    return tileAnswer;
  }

  @override
  Future<bool> requestPinWidget() async => true;

  @override
  Future<AndroidQuickAccessState> setPanelOption(
    AndroidPanelOption option,
    bool enabled,
  ) async => current = _state(
    keyboardEnabled: current.keyboardEnabled,
    keyboardSelected: current.keyboardSelected,
    tileRequestSupported: current.tileRequestSupported,
    autoReturn: option == AndroidPanelOption.autoReturn
        ? enabled
        : current.option(AndroidPanelOption.autoReturn),
  );
}

AndroidQuickAccessState _state({
  bool keyboardEnabled = false,
  bool keyboardSelected = false,
  bool tileRequestSupported = true,
  bool autoReturn = true,
}) => AndroidQuickAccessState(
  keyboardEnabled: keyboardEnabled,
  keyboardSelected: keyboardSelected,
  backgroundCapture: false,
  tileRequestSupported: tileRequestSupported,
  widgetPinSupported: true,
  widgetCount: 1,
  panelOptions: {AndroidPanelOption.autoReturn: autoReturn},
);

void main() {
  final l10n = lookupAppLocalizations(AppLocalization.fallback);

  Future<QuickAccessController> pump(
    WidgetTester tester,
    _FakeGateway gateway,
  ) async {
    final controller = QuickAccessController(gateway);
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        builder: AppTheme.builder,
        home: Scaffold(
          child: SingleChildScrollView(
            child: QuickAccessSettingsView(
              controller: controller,
              targets: QuickAccessSettingsTargets(),
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    return controller;
  }

  testWidgets('rows reflect the gateway state', (tester) async {
    final gateway = _FakeGateway(
      _state(keyboardEnabled: true, keyboardSelected: true),
    );
    await pump(tester, gateway);

    expect(find.text(l10n.quickAccessKeyboardOn), findsOneWidget);
    expect(find.text(l10n.quickAccessKeyboardCurrent), findsOneWidget);
    expect(find.text(l10n.quickAccessTierPanelCollecting), findsOneWidget);
    expect(find.text(l10n.quickAccessWidgetCount(1)), findsOneWidget);

    final autoReturn = find.byKey(
      const ValueKey<String>('quick-access-option-autoReturn'),
    );
    await tester.ensureVisible(autoReturn);
    await tester.tap(autoReturn);
    await tester.pumpAndSettle();
    expect(gateway.current.option(AndroidPanelOption.autoReturn), isFalse);

    gateway.current = _state();
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.inactive);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.resumed);
    await tester.pumpAndSettle();
    expect(find.text(l10n.quickAccessKeyboardOff), findsOneWidget);
    expect(find.text(l10n.quickAccessTierManual), findsOneWidget);
    final picker = tester.widget<Button>(
      find.byKey(const ValueKey<String>('quick-access-keyboard-picker')),
    );
    expect(picker.onPressed, isNull);
  });

  testWidgets(
    'below Android 13 the tile rows give instructions instead of a prompt',
    (tester) async {
      await pump(tester, _FakeGateway(_state(tileRequestSupported: false)));

      expect(find.text(l10n.quickAccessTilesTitle), findsOneWidget);
      expect(find.text(l10n.quickAccessTileInstructions), findsOneWidget);
      expect(find.text(l10n.quickAccessAddTile), findsNothing);
    },
  );

  testWidgets('a denied tile prompt is not offered again', (tester) async {
    final gateway = _FakeGateway(_state());
    await pump(tester, gateway);

    final add = find.byKey(
      const ValueKey<String>('quick-access-add-tile-saveClipboard'),
    );
    await tester.ensureVisible(add);
    await tester.tap(add);
    await tester.pumpAndSettle();

    expect(gateway.tileRequests, [AndroidQuickAccessTile.saveClipboard]);
    expect(add, findsNothing);
    expect(find.text(l10n.quickAccessTileInstructions), findsOneWidget);
    expect(
      find.byKey(const ValueKey<String>('quick-access-add-tile-captureToggle')),
      findsOneWidget,
    );
  });
}
