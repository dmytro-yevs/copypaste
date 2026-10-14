import 'dart:async';

import 'package:copypaste_flutter/platform/desktop/desktop_visibility_controller.dart';
import 'package:copypaste_flutter/platform/desktop/desktop_visibility_preferences.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';

void main() {
  test(
    'fresh installs show both icons and restore independent choices',
    () async {
      final storage = _Preferences();
      final store = SharedPreferencesDesktopVisibilityStore(
        preferences: storage,
      );
      final defaults = await store.read();
      expect(defaults.showInTaskbar, isTrue);
      expect(defaults.showInTray, isTrue);
      await store.write(
        const DesktopVisibilityPreferences(showInTaskbar: false),
      );
      final restored = await SharedPreferencesDesktopVisibilityStore(
        preferences: storage,
      ).read();
      expect(restored.showInTaskbar, isFalse);
      expect(restored.showInTray, isTrue);
      storage.encoded = '{"showInTray":false}';
      expect((await store.read()).showInTaskbar, isTrue);
      expect((await store.read()).showInTray, isFalse);
      for (final corrupt in ['broken', '[]', '{"showInTray":"false"}']) {
        storage.encoded = corrupt;
        final value = await store.read();
        expect(value.showInTaskbar, isTrue);
        expect(value.showInTray, isTrue);
      }
    },
  );

  test(
    'applies both hidden and re-enables either icon independently',
    () async {
      final host = _Host();
      final store = MemoryDesktopVisibilityStore();
      final controller = DesktopVisibilityController(host: host, store: store);
      addTearDown(controller.dispose);
      await controller.load();
      expect(await controller.setTaskbarVisible(false), isTrue);
      expect(await controller.setTrayVisible(false), isTrue);
      expect(host.taskbar, isFalse);
      expect(host.tray, isFalse);
      expect(store.value.showInTaskbar, isFalse);
      expect(store.value.showInTray, isFalse);
      expect(await controller.setTrayVisible(true), isTrue);
      expect(host.taskbar, isFalse);
      expect(host.tray, isTrue);
      expect(await controller.setTaskbarVisible(true), isTrue);
      expect(store.value.showInTaskbar, isTrue);
    },
  );

  test('native failure retains saved preferences and supports retry', () async {
    final host = _Host()..failTray = true;
    final store = MemoryDesktopVisibilityStore();
    final controller = DesktopVisibilityController(host: host, store: store);
    addTearDown(controller.dispose);
    expect(await controller.setTrayVisible(false), isFalse);
    expect(controller.preferences.showInTray, isTrue);
    expect(store.value.showInTray, isTrue);
    expect(controller.error, isNotNull);
    expect(controller.busy, isFalse);
    host.failTray = false;
    expect(await controller.setTrayVisible(false), isTrue);
    expect(controller.error, isNull);
  });

  test('storage failure rolls the native change back', () async {
    final host = _Host();
    final controller = DesktopVisibilityController(
      host: host,
      store: _FailingStore(),
    );
    addTearDown(controller.dispose);
    expect(await controller.setTaskbarVisible(false), isFalse);
    expect(host.taskbar, isTrue);
    expect(controller.preferences.showInTaskbar, isTrue);
    expect(controller.error, isNotNull);
  });

  test(
    'rejects overlapping updates and settles before native disposal',
    () async {
      final host = _Host()..gate = Completer<void>();
      final store = MemoryDesktopVisibilityStore();
      final controller = DesktopVisibilityController(host: host, store: store);
      addTearDown(controller.dispose);
      final update = controller.setTaskbarVisible(false);
      expect(controller.busy, isTrue);
      expect(await controller.setTrayVisible(false), isFalse);
      var settled = false;
      final settling = controller.settle().then((_) => settled = true);
      await Future<void>.delayed(Duration.zero);
      expect(settled, isFalse);
      host.gate!.complete();
      expect(await update, isTrue);
      await settling;
      expect(store.value.showInTaskbar, isFalse);
      expect(store.value.showInTray, isTrue);
    },
  );
}

class _Host implements DesktopVisibilityHost {
  bool taskbar = true;
  bool tray = true;
  bool failTray = false;
  Completer<void>? gate;

  @override
  Future<void> setTaskbarVisible(bool visible) async {
    await gate?.future;
    taskbar = visible;
  }

  @override
  Future<void> setTrayVisible(bool visible) async {
    if (failTray) throw StateError('Native tray failure');
    tray = visible;
  }
}

class _FailingStore extends MemoryDesktopVisibilityStore {
  @override
  Future<void> write(DesktopVisibilityPreferences preferences) async {
    throw StateError('Storage failure');
  }
}

class _Preferences implements SharedPreferencesAsync {
  final Map<String, String> _values = {};
  String? get encoded => _values['value'];
  set encoded(String value) => _values['value'] = value;

  @override
  Future<String?> getString(String key) async => encoded;

  @override
  Future<void> setString(String key, String value) async => encoded = value;

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}
