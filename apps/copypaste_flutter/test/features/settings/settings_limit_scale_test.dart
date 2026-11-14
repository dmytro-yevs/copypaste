import 'dart:async';

import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/settings/controller/settings_controller.dart';
import 'package:copypaste_flutter/features/settings/models/settings_models.dart';
import 'package:copypaste_flutter/features/settings/view/settings_screen.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'settings_test_support.dart';

const _gib = 1024 * 1024 * 1024;

void main() {
  Future<(SettingsController, _Repository)> pumpSettings(
    WidgetTester tester, {
    Size size = const Size(1200, 900),
    double textScale = 1,
    StorageUsage? usage,
  }) async {
    await tester.binding.setSurfaceSize(size);
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final repository = _Repository();
    if (usage != null) {
      repository.currentSettings = RuntimeSettings(
        retentionDays: 0,
        storageQuotaBytes: usage.quotaBytes,
        excludedAppIds: const [],
        lanVisibility: true,
        syncEnabled: true,
        notifyOnCopy: false,
        soundOnCopy: false,
        storageUsage: usage,
      );
    }
    final controller = SettingsController(
      screenshotProtection: FakeScreenshotProtection(),
      repository: repository,
      filePicker: FakeSettingsFilePicker(),
      notifications: FakeCaptureNotificationPort(),
      captureRefreshInterval: Duration.zero,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        builder: (context, child) => MediaQuery(
          data: MediaQuery.of(
            context,
          ).copyWith(textScaler: TextScaler.linear(textScale)),
          child: AppTheme.builder(context, child),
        ),
        home: Scaffold(child: SettingsScreen(controller: controller)),
      ),
    );
    await tester.pump();
    return (controller, repository);
  }

  Finder scale(String name) => find.byKey(ValueKey<String>('settings-$name'));
  Finder track(String name) =>
      find.descendant(of: scale(name), matching: find.byType(Slider));

  Future<void> focusScale(WidgetTester tester, String name) async {
    final thumb = find.descendant(
      of: find.descendant(
        of: scale(name),
        matching: find.byType(FocusableActionDetector),
      ),
      matching: find.byType(Container),
    );
    Focus.of(tester.element(thumb.first)).requestFocus();
    await tester.pump();
  }

  testWidgets(
    'a stricter retention asks first with Cancel focused and reverts',
    (tester) async {
      final semantics = tester.ensureSemantics();
      final (_, repository) = await pumpSettings(tester);
      final retention = scale('retention-scale');
      await tester.ensureVisible(retention);
      expect(
        tester.getSemantics(track('retention-scale')),
        isSemantics(
          isSlider: true,
          label: 'Retention',
          value: 'Keep forever',
          hasIncreaseAction: false,
          hasDecreaseAction: true,
        ),
      );

      await focusScale(tester, 'retention-scale');
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowLeft);
      await tester.pumpAndSettle();
      expect(find.text('Remove older history?'), findsOneWidget);
      expect(find.textContaining('New limit: 365 days.'), findsOneWidget);
      expect(
        FocusManager.instance.primaryFocus?.debugLabel,
        'confirmation-cancel',
      );
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(find.text('Remove older history?'), findsNothing);
      expect(repository.currentSettings.retentionDays, 0);
      expect(
        tester.getSemantics(track('retention-scale')),
        isSemantics(value: 'Keep forever'),
      );

      await focusScale(tester, 'retention-scale');
      await tester.sendKeyEvent(LogicalKeyboardKey.home);
      await tester.pumpAndSettle();
      await tester.tap(find.text('Apply limit'));
      await tester.pumpAndSettle();
      expect(repository.currentSettings.retentionDays, 7);
      expect(
        tester.getSemantics(track('retention-scale')),
        isSemantics(value: '7 days'),
      );

      // Loosening a limit deletes nothing and applies without asking.
      await focusScale(tester, 'retention-scale');
      await tester.sendKeyEvent(LogicalKeyboardKey.end);
      await tester.pumpAndSettle();
      expect(find.text('Remove older history?'), findsNothing);
      expect(repository.currentSettings.retentionDays, 0);
      expect(tester.takeException(), isNull);
      semantics.dispose();
      await tester.pumpAndSettle();
    },
    variant: TargetPlatformVariant.only(TargetPlatform.macOS),
  );

  testWidgets('a smaller quota asks first; the file limit never does', (
    tester,
  ) async {
    final (_, repository) = await pumpSettings(tester);
    await tester.ensureVisible(scale('storage-quota-scale'));
    await focusScale(tester, 'storage-quota-scale');
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowLeft);
    await tester.pumpAndSettle();
    expect(find.text('Reduce the storage quota?'), findsOneWidget);
    expect(find.textContaining('New quota: 5 GiB.'), findsOneWidget);
    await tester.tap(find.text('Apply limit'));
    await tester.pumpAndSettle();
    expect(repository.currentSettings.storageQuotaBytes, 5 * _gib);

    await tester.ensureVisible(scale('max-file-size-scale'));
    await focusScale(tester, 'max-file-size-scale');
    await tester.sendKeyEvent(LogicalKeyboardKey.home);
    await tester.pumpAndSettle();
    expect(find.byType(AlertDialog), findsNothing);
    expect(repository.currentSettings.maxFileSizeBytes, 1024 * 1024);
    expect(tester.takeException(), isNull);
  }, variant: TargetPlatformVariant.only(TargetPlatform.windows));

  testWidgets(
    'the quota row reports usage and names what a lower quota removes',
    (tester) async {
      final (controller, _) = await pumpSettings(
        tester,
        usage: StorageUsage(
          usedBytes: 2254857830,
          quotaBytes: 10 * _gib,
          oldestUnpinnedAt: DateTime(2026, 8, 14),
        ),
      );
      final usage = find.byKey(
        const ValueKey<String>('settings-storage-usage'),
      );
      expect(
        tester.widget<Text>(usage).data,
        startsWith('2.1 GiB used of 10 GiB · Oldest clip '),
      );

      // A quota below the usage says the oldest clips go, with the numbers.
      await tester.ensureVisible(scale('storage-quota-scale'));
      await focusScale(tester, 'storage-quota-scale');
      await tester.sendKeyEvent(LogicalKeyboardKey.home);
      await tester.pumpAndSettle();
      expect(
        find.textContaining(
          'New quota: 1 GiB. Unpinned history uses 2.1 GiB, so its oldest '
          'clips are removed now',
        ),
        findsOneWidget,
      );
      await tester.tap(find.text('Cancel'));
      await tester.pumpAndSettle();

      // Hidden Settings stops watching its readout.
      controller.setPresentationActive(false);
      await tester.pump();
      expect(find.byType(ValueListenableBuilder<StorageUsage?>), findsNothing);
      expect(usage, findsOneWidget);
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant.only(TargetPlatform.windows),
  );

  testWidgets('a dragged scale applies once, on release', (tester) async {
    final (_, repository) = await pumpSettings(tester);
    final size = scale('max-file-size-scale');
    await tester.ensureVisible(size);
    await tester.pump();
    final track = find.descendant(of: size, matching: find.byType(Slider));
    final start = tester.getCenter(track);
    final gesture = await tester.startGesture(start);
    await gesture.moveBy(const Offset(40, 0));
    await tester.pump();
    await gesture.moveBy(Offset(tester.getSize(track).width / 2, 0));
    await tester.pump();
    expect(repository.writes, 0);
    // The value beside the copy follows the thumb; the end stop is named too.
    expect(
      find.descendant(of: size, matching: find.text('20 GiB')),
      findsNWidgets(2),
    );
    await gesture.up();
    await tester.pumpAndSettle();
    expect(repository.writes, 1);
    expect(repository.currentSettings.maxFileSizeBytes, 20 * _gib);
  }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));

  testWidgets('on Android a 48 px band around the track takes taps and drags', (
    tester,
  ) async {
    final (_, repository) = await pumpSettings(
      tester,
      size: const Size(390, 844),
    );
    await tester.tap(
      find.byKey(const ValueKey('mobile-settings-section-clipboard')),
    );
    await tester.pumpAndSettle();
    final size = scale('max-file-size-scale');
    await tester.ensureVisible(size);
    await tester.pumpAndSettle();
    final track = find.descendant(of: size, matching: find.byType(Slider));
    final band = find.ancestor(
      of: track,
      matching: find.byWidgetPredicate(
        (widget) => widget.runtimeType.toString() == '_TouchTarget',
      ),
    );
    final bandRect = tester.getRect(band);
    final trackRect = tester.getRect(track);
    expect(bandRect.height, 48);
    // The stock slider keeps its compact look inside the band.
    expect(trackRect.height, lessThan(bandRect.height));
    // The band is laid out inside its own row, below the row above.
    final rowAbove = find.byKey(
      const ValueKey<String>('settings-row-Storage quota'),
    );
    expect(bandRect.top, greaterThanOrEqualTo(tester.getRect(rowAbove).bottom));

    // A tap above the track, inside the band, picks the stop under it.
    await tester.tapAt(Offset(trackRect.right - 8, bandRect.top + 2));
    await tester.pumpAndSettle();
    expect(repository.currentSettings.maxFileSizeBytes, 20 * _gib);

    // A drag that starts below the track moves the thumb.
    final gesture = await tester.startGesture(
      Offset(trackRect.right - 8, bandRect.bottom - 2),
    );
    await gesture.moveBy(const Offset(-30, 0));
    await tester.pump();
    await gesture.moveTo(Offset(trackRect.left + 8, bandRect.bottom - 2));
    await tester.pump();
    await gesture.up();
    await tester.pumpAndSettle();
    // The stock slider follows drag deltas, so touch slop is not travelled.
    expect(
      repository.currentSettings.maxFileSizeBytes,
      lessThan(64 * 1024 * 1024),
    );
    expect(tester.takeException(), isNull);
  }, variant: TargetPlatformVariant.only(TargetPlatform.android));

  testWidgets('a scale names its value beside the copy and only its ends', (
    tester,
  ) async {
    await pumpSettings(tester);
    final quota = scale('storage-quota-scale');
    await tester.ensureVisible(quota);
    await tester.pumpAndSettle();
    Rect textRect(String text) =>
        tester.getRect(find.descendant(of: quota, matching: find.text(text)));
    final title = textRect('Storage quota');
    final value = textRect('10 GiB');
    expect(value.left, greaterThan(title.right));
    expect(
      value.bottom,
      lessThan(tester.getRect(track('storage-quota-scale')).top),
    );
    for (final stop in ['1 GiB', '50 GiB']) {
      expect(find.descendant(of: quota, matching: find.text(stop)), findsOne);
    }
    expect(
      find.descendant(of: quota, matching: find.text('5 GiB')),
      findsNothing,
    );
    expect(tester.takeException(), isNull);
  }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));

  testWidgets(
    'scales keep their value text at a narrow width and doubled text',
    (tester) async {
      await pumpSettings(tester, size: const Size(320, 720), textScale: 2);
      await tester.tap(
        find.byKey(const ValueKey('mobile-settings-section-clipboard')),
      );
      await tester.pumpAndSettle();
      await tester.ensureVisible(scale('storage-quota-scale'));
      await tester.pumpAndSettle();
      expect(
        find.descendant(
          of: scale('storage-quota-scale'),
          matching: find.text('10 GiB'),
        ),
        findsOneWidget,
      );
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant.only(TargetPlatform.android),
  );

  testWidgets('Tab leaves the search field and keeps moving', (tester) async {
    await pumpSettings(tester);
    await tester.tap(find.byKey(const ValueKey<String>('settings-search')));
    await tester.pump();
    final visited = <FocusNode>{};
    for (var step = 0; step < 4; step++) {
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pump();
      final focus = FocusManager.instance.primaryFocus!;
      expect(focus, isNot(isA<FocusScopeNode>()));
      expect(
        focus.context?.findAncestorWidgetOfExactType<TextField>()?.key,
        isNot(const ValueKey<String>('settings-search')),
      );
      visited.add(focus);
    }
    expect(visited, hasLength(4));
    await tester.pumpAndSettle();
  }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));

  testWidgets('only the switch being saved is disabled, then refocused', (
    tester,
  ) async {
    final (_, repository) = await pumpSettings(tester);
    await tester.tap(
      find.descendant(
        of: find.byKey(const ValueKey<String>('settings-section-sync')),
        matching: find.text('Sync'),
      ),
    );
    await tester.pumpAndSettle();
    Finder rowSwitch(String title) => find.descendant(
      of: find.byKey(ValueKey<String>('settings-row-$title')),
      matching: find.byType(Switch),
    );
    FocusNode switchFocus(String title) => Focus.of(
      tester.element(
        find
            .descendant(
              of: find.descendant(
                of: rowSwitch(title),
                matching: find.byType(FocusableActionDetector),
              ),
              matching: find.byType(Row),
            )
            .first,
      ),
    );
    switchFocus('Sync').requestFocus();
    await tester.pump();
    repository.gate = Completer<void>();
    await tester.sendKeyEvent(LogicalKeyboardKey.space);
    await tester.pump();

    expect(tester.widget<Switch>(rowSwitch('Sync')).enabled, isFalse);
    expect(tester.widget<Switch>(rowSwitch('LAN visibility')).enabled, isTrue);
    expect(
      tester.getSemantics(rowSwitch('Sync')),
      isSemantics(isEnabled: false, label: 'Sync'),
    );

    repository.gate!.complete();
    await tester.pumpAndSettle();
    expect(repository.currentSettings.syncEnabled, isFalse);
    expect(tester.widget<Switch>(rowSwitch('Sync')).enabled, isTrue);
    expect(FocusManager.instance.primaryFocus, switchFocus('Sync'));
  }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));

  testWidgets('the failure banner can be dismissed', (tester) async {
    final (controller, repository) = await pumpSettings(tester);
    repository.failWrites = true;
    expect(await controller.setMaxFileSizeBytes(4 * 1024 * 1024), isFalse);
    await tester.pump();
    expect(find.text('Settings need attention'), findsOneWidget);
    await tester.tap(
      find.byKey(const ValueKey<String>('settings-error-dismiss')),
    );
    await tester.pump();
    expect(find.text('Settings need attention'), findsNothing);
    await tester.pumpAndSettle();
  }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));

  testWidgets('export reports the saved and unreadable clips in one toast', (
    tester,
  ) async {
    final (_, repository) = await pumpSettings(tester);
    repository.exportSkippedUndecryptable = 3;
    await tester.tap(
      find.descendant(
        of: find.byKey(const ValueKey<String>('settings-section-data')),
        matching: find.text('Data'),
      ),
    );
    await tester.pump();
    await tester.tap(find.text('Export'));
    await tester.pump();
    expect(find.text('Text history exported'), findsOneWidget);
    expect(find.text('Exported 4 text clips · 3 could not be read'), findsOne);
    await tester.pumpWidget(const SizedBox.shrink());
    await tester.pump(const Duration(seconds: 10));
  }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));
}

class _Repository extends FakeSettingsRepository {
  bool failWrites = false;
  int writes = 0;
  Completer<void>? gate;

  @override
  Future<RuntimeSettings> updateSettings(RuntimeSettingsChange change) async {
    writes += 1;
    await gate?.future;
    if (failWrites) throw StateError('write failed');
    return super.updateSettings(change);
  }
}
