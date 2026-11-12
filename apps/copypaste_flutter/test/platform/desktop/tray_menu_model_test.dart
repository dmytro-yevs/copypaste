import 'package:copypaste_flutter/features/tray/controller/tray_menu_controller.dart';
import 'package:copypaste_flutter/features/tray/models/tray_menu_model.dart';
import 'package:copypaste_flutter/features/tray/repository/tray_clip_source.dart';
import 'package:copypaste_flutter/features/tray/repository/tray_preferences_store.dart';
import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:copypaste_flutter/platform/capture/capture_control_state.dart';
import 'package:copypaste_flutter/platform/security/device_authenticator.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';

final _l10n = lookupAppLocalizations(const Locale('en'));

void main() {
  test('the menu offers every quick action, Recent and New snippet', () {
    final menu = buildTrayMenu(
      const TrayMenuState(
        capture: CaptureControlState.running,
        quickPasteShortcut: '⌘+Shift+C',
        clipsAvailable: true,
      ),
      _l10n,
    );
    final keys = _keys(menu);

    expect(
      keys,
      containsAll([
        TrayMenuKeys.toggleCapture,
        TrayMenuKeys.pauseFor,
        TrayMenuKeys.captureNow,
        TrayMenuKeys.openQuickPaste,
        TrayMenuKeys.copyLatest,
        TrayMenuKeys.recent,
        TrayMenuKeys.openApp,
        TrayMenuKeys.newSnippet,
      ]),
    );
    expect(
      _item(menu, TrayMenuKeys.openQuickPaste).label,
      'Open Quick Paste (⌘+Shift+C)',
    );
    final pauseFor = menu.whereType<TrayMenuSubmenu>().firstWhere(
      (entry) => entry.key == TrayMenuKeys.pauseFor,
    );
    expect(
      pauseFor.children.whereType<TrayMenuItem>().map((item) => item.action),
      const [
        TrayPauseAction(Duration(minutes: 15)),
        TrayPauseAction(Duration(hours: 1)),
        TrayPauseAction(null),
      ],
    );
  });

  test(
    'Recent loads on open, masks secrets and is released on close',
    () async {
      final surface = _FakeSurface();
      final source = _FakeSource();
      final controller = _controller(surface, source, _FakeAuthenticator());
      await controller.start();

      expect(source.recentReads, 0);
      expect(_recentLabels(surface), ['Loading…']);

      controller.trayMenuOpened();
      await pumpEventQueue();

      expect(source.recentReads, 1);
      final labels = _recentLabels(surface);
      expect(labels.first, 'first line second line');
      expect(labels.last, startsWith('Secret · Vault · '));
      expect(labels.join('\n'), isNot(contains('hunter2')));

      controller.trayMenuClosed();
      await Future<void>.delayed(
        TrayMenuController.releaseDelay + const Duration(milliseconds: 50),
      );
      expect(surface.trayState.recent, isEmpty);
      expect(_recentLabels(surface), ['Loading…']);
      await controller.dispose();
    },
  );

  test('copying a secret needs a successful device authentication', () async {
    final surface = _FakeSurface();
    final source = _FakeSource();
    final authenticator = _FakeAuthenticator()
      ..results.addAll([DeviceAuthResult.cancelled, DeviceAuthResult.success]);
    final controller = _controller(surface, source, authenticator);
    await controller.start();
    controller.trayMenuOpened();
    await pumpEventQueue();

    await controller.trayActionSelected(const TrayCopyClipAction('secret'));
    expect(source.copied, isEmpty);
    await controller.trayActionSelected(const TrayCopyClipAction('secret'));
    expect(source.copied, ['secret']);
    await controller.trayActionSelected(const TrayCopyClipAction('plain'));
    expect(source.copied, ['secret', 'plain']);
    expect(authenticator.prompts, 2);
    await controller.dispose();
  });

  test('the paste queue slot follows its count and collecting state', () async {
    final surface = _FakeSurface();
    final controller = _controller(
      surface,
      _FakeSource(),
      _FakeAuthenticator(),
    );
    await controller.start();
    expect(_keys(surface.menu), isNot(contains(TrayMenuKeys.pasteQueue)));

    await controller.setPasteQueue(
      const TrayQueueStatus(count: 3, collecting: false),
    );
    expect(
      _item(surface.menu, TrayMenuKeys.pasteQueue).label,
      'Paste Queue: 3 items',
    );

    await controller.setPasteQueue(
      const TrayQueueStatus(count: 1, collecting: true),
    );
    expect(
      _item(surface.menu, TrayMenuKeys.pasteQueue).label,
      'Collecting: 1 item',
    );
    await controller.dispose();
  });
}

TrayMenuController _controller(
  _FakeSurface surface,
  _FakeSource source,
  _FakeAuthenticator authenticator,
) => TrayMenuController(
  surface: surface,
  source: source,
  authenticator: authenticator,
  preferencesStore: MemoryTrayPreferencesStore(),
  loadOnOpen: true,
);

List<String> _keys(List<TrayMenuEntry> menu) => [
  for (final entry in menu) entry.key,
];

TrayMenuItem _item(List<TrayMenuEntry> menu, String key) =>
    menu.whereType<TrayMenuItem>().firstWhere((item) => item.key == key);

List<String> _recentLabels(_FakeSurface surface) => surface.menu
    .whereType<TrayMenuSubmenu>()
    .firstWhere((entry) => entry.key == TrayMenuKeys.recent)
    .children
    .whereType<TrayMenuItem>()
    .map((item) => item.label)
    .toList();

class _FakeSurface implements TrayMenuSurface {
  @override
  TrayMenuState trayState = const TrayMenuState(
    capture: CaptureControlState.running,
  );
  TrayMenuDelegate? delegate;

  List<TrayMenuEntry> get menu => buildTrayMenu(trayState, _l10n);

  @override
  void setTrayMenuDelegate(TrayMenuDelegate? delegate) {
    this.delegate = delegate;
  }

  @override
  Future<void> showFromTrayOrDock() async {}

  @override
  Future<void> updateTrayMenu(
    TrayMenuState Function(TrayMenuState state) update,
  ) async {
    trayState = update(trayState);
  }
}

class _FakeSource implements TrayClipSource {
  int recentReads = 0;
  final copied = <String>[];

  @override
  Future<List<TrayClip>> recent(int limit) async {
    recentReads += 1;
    return [
      TrayClip(
        id: 'plain',
        kind: TrayClipKind.text,
        masked: false,
        capturedAt: DateTime(2026, 11, 12, 9, 30),
        preview: '  first line\n\tsecond line ',
      ),
      TrayClip(
        id: 'secret',
        kind: TrayClipKind.text,
        masked: true,
        capturedAt: DateTime(2026, 11, 12, 9, 31),
        sourceApp: 'Vault',
      ),
    ];
  }

  @override
  Future<TrayClip?> latestFromOtherDevice() async => null;

  @override
  Future<void> copy(String id) async => copied.add(id);

  @override
  Future<bool> captureClipboard() async => false;

  @override
  Future<List<TraySkipEntry>> recentlySkipped() async => const [];
}

class _FakeAuthenticator implements DeviceAuthenticator {
  final results = <DeviceAuthResult>[];
  int prompts = 0;

  @override
  Future<DeviceAuthAvailability> availability() async =>
      DeviceAuthAvailability.available;

  @override
  Future<DeviceAuthResult> authenticate({required String reason}) async {
    prompts += 1;
    expect(reason, isNotEmpty);
    return results.isEmpty ? DeviceAuthResult.success : results.removeAt(0);
  }
}
