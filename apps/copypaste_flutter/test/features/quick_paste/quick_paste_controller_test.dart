import 'package:copypaste_flutter/features/history/models/history_models.dart';
import 'package:copypaste_flutter/features/history/presentation/history_identity_label.dart';
import 'package:copypaste_flutter/features/history/repository/history_repository.dart';
import 'package:copypaste_flutter/features/quick_paste/quick_paste_app.dart';
import 'package:copypaste_flutter/features/quick_paste/quick_paste_controller.dart';
import 'package:copypaste_flutter/features/settings/repository/quick_paste_preferences_store.dart';
import 'package:copypaste_flutter/platform/desktop/global_shortcut.dart';
import 'package:copypaste_flutter/platform/desktop/quick_paste_host.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('renders the keyboard-first popup and footer actions', (
    tester,
  ) async {
    final controller = QuickPasteController(
      repository: _Repository(),
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: _ContextHost()..permissionGranted = true,
    );

    await tester.binding.setSurfaceSize(const Size(520, 720));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(QuickPasteApp(controller: controller));
    await tester.pumpAndSettle(const Duration(milliseconds: 150));

    expect(find.text('Type to search…'), findsOneWidget);
    expect(find.text('Pinned clip'), findsOneWidget);
    expect(find.text('Recent clip'), findsOneWidget);
    final sourceApps = find.byType(HistoryIdentityLabel);
    expect(sourceApps, findsNWidgets(2));
    expect(
      tester
          .widgetList<HistoryIdentityLabel>(sourceApps)
          .map((label) => label.name),
      everyElement('Editor'),
    );
    expect(find.text('Clear unpinned'), findsOneWidget);
    expect(find.text('Settings'), findsOneWidget);
    expect(find.text('About'), findsOneWidget);
    expect(find.text('Quit'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  test('auto-pastes the copied clip when permission is available', () async {
    final repository = _Repository();
    final host = _ContextHost()..permissionGranted = true;
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: host,
    );
    addTearDown(controller.dispose);
    await controller.initialize();

    final clip = controller.history.items.last;
    controller.focus(clip);
    await controller.activateFocused();

    expect(repository.copied, ['recent']);
    expect(host.pasteCalls, 1);
    expect(host.closeCalls, 0);
  });

  test('falls back to copy and close when Accessibility is denied', () async {
    final repository = _Repository();
    final host = _ContextHost();
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: host,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await controller.opened();

    await controller.activate(controller.history.items.last);

    expect(host.permissionRequests, 1);
    expect(repository.copied, ['recent']);
    expect(host.pasteCalls, 0);
    expect(host.closeCalls, 1);
  });

  test('Option selection inverts copy-only mode for one paste', () async {
    final repository = _Repository();
    final host = _ContextHost()..permissionGranted = true;
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(
        QuickPastePreferences(
          autoPaste: false,
          shortcut: DesktopShortcut.defaultForPlatform(),
        ),
      ),
      host: host,
    );
    addTearDown(controller.dispose);
    await controller.initialize();

    await controller.activate(
      controller.history.items.last,
      invertAutoPaste: true,
    );

    expect(host.pasteCalls, 1);
    expect(host.closeCalls, 0);
  });

  test('clear keeps pinned clips visible', () async {
    final repository = _Repository();
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: _ContextHost()..permissionGranted = true,
    );
    addTearDown(controller.dispose);
    await controller.initialize();

    expect(await controller.clearUnpinned(), isTrue);
    expect(controller.history.items.map((clip) => clip.id), ['pinned']);
    expect(repository.deleteAllCalls, 1);
  });
}

class _Repository implements HistoryRepository {
  final clips = <HistoryClip>[
    HistoryClip(
      id: 'pinned',
      contentType: 'text/plain',
      preview: 'Pinned clip',
      createdAt: DateTime.utc(2026),
      pinned: true,
      sourceApp: 'Editor',
    ),
    HistoryClip(
      id: 'recent',
      contentType: 'text/plain',
      preview: 'Recent clip',
      createdAt: DateTime.utc(2026),
      pinned: false,
      sourceApp: 'Editor',
    ),
  ];
  final copied = <String>[];
  int deleteAllCalls = 0;

  @override
  Future<void> copy(String id) async {
    copied.add(id);
  }

  @override
  Future<void> copyPlainText(String id) => copy(id);

  @override
  Future<void> saveFile(String id, String destinationPath) async {}

  @override
  Future<void> delete(String id) async {
    clips.removeWhere((clip) => clip.id == id);
  }

  @override
  Future<void> deleteAll() async {
    deleteAllCalls += 1;
    clips.removeWhere((clip) => !clip.pinned);
  }

  @override
  Future<HistoryFacets> facets() async => const HistoryFacets();

  @override
  Future<HistoryClip> get(String id) async =>
      clips.singleWhere((clip) => clip.id == id);

  @override
  Future<HistoryImagePreview?> imagePreview(String id, {int? maxEdge}) async =>
      null;

  @override
  Future<HistoryClipPage> query({
    required HistoryQuery query,
    required int limit,
    String? cursor,
  }) async => HistoryClipPage(items: List.of(clips));

  @override
  Future<void> reorderPinned(List<String> ids) async {}

  @override
  Future<void> setPinned(String id, bool pinned) async {}

  @override
  Future<HistorySourceAppIcon?> sourceAppIcon(String id) async => null;

  @override
  Stream<HistoryRuntimeEvent> watch() => const Stream.empty();
}

class _ContextHost implements QuickPasteContextHost {
  bool permissionGranted = false;
  int permissionRequests = 0;
  int pasteCalls = 0;
  int closeCalls = 0;
  Future<void> Function()? openedHandler;

  @override
  Future<bool> accessibilityGranted() async => permissionGranted;

  @override
  Future<void> close() async {
    closeCalls += 1;
  }

  @override
  Future<void> dispose() async {}

  @override
  Future<void> openMainWindow() async {}

  @override
  Future<void> openSettings() async {}

  @override
  Future<bool> paste() async {
    pasteCalls += 1;
    return true;
  }

  @override
  Future<void> quit() async {}

  @override
  Future<bool> requestAccessibility() async {
    permissionRequests += 1;
    return permissionGranted;
  }

  @override
  void setOpenedHandler(Future<void> Function()? handler) {
    openedHandler = handler;
  }
}
