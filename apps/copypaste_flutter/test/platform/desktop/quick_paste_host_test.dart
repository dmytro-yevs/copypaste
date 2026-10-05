import 'package:copypaste_flutter/features/settings/controller/quick_paste_settings_controller.dart';
import 'package:copypaste_flutter/features/settings/repository/quick_paste_preferences_store.dart';
import 'package:copypaste_flutter/platform/desktop/global_shortcut.dart';
import 'package:copypaste_flutter/platform/desktop/quick_paste_host.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('test/quick_paste_host');
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
  late MethodChannelQuickPasteWindowHost host;
  Object? response;

  setUp(() {
    response = true;
    messenger.setMockMethodCallHandler(channel, (call) async => response);
    host = MethodChannelQuickPasteWindowHost(channel: channel);
  });

  tearDown(() async {
    await host.dispose();
    messenger.setMockMethodCallHandler(channel, null);
    debugDefaultTargetPlatformOverride = null;
  });

  for (final method in ['prepare', 'open']) {
    test('$method accepts native true', () async {
      await (method == 'prepare' ? host.prepare() : host.open());
    });

    test('$method rejects native false', () async {
      response = false;
      await expectLater(
        method == 'prepare' ? host.prepare() : host.open(),
        throwsA(
          isA<PlatformException>().having(
            (error) => error.code,
            'code',
            'window_unavailable',
          ),
        ),
      );
    });

    test('$method preserves native platform errors', () async {
      messenger.setMockMethodCallHandler(channel, (call) async {
        if (call.method == method) {
          throw PlatformException(code: 'native_failure');
        }
        return true;
      });
      await expectLater(
        method == 'prepare' ? host.prepare() : host.open(),
        throwsA(
          isA<PlatformException>().having(
            (error) => error.code,
            'code',
            'native_failure',
          ),
        ),
      );
    });
  }

  test('macOS open retains nullable native success', () async {
    debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
    response = null;
    await host.open();
  });

  test('Windows open rejects null instead of reporting success', () async {
    debugDefaultTargetPlatformOverride = TargetPlatform.windows;
    response = null;
    await expectLater(host.open(), throwsA(isA<PlatformException>()));
  });

  test('Windows open rejects malformed native success', () async {
    debugDefaultTargetPlatformOverride = TargetPlatform.windows;
    response = 'success';
    await expectLater(host.open(), throwsA(isA<TypeError>()));
  });

  test('prepare requires confirmed native success', () async {
    response = null;
    await expectLater(host.prepare(), throwsA(isA<PlatformException>()));
  });

  test('false prepare stops shortcut registration', () async {
    messenger.setMockMethodCallHandler(
      channel,
      (call) async => call.method != 'prepare',
    );
    final registrar = _Registrar();
    final controller = QuickPasteSettingsController(
      store: MemoryQuickPastePreferencesStore(),
      registrar: registrar,
      windowHost: host,
    );
    addTearDown(controller.dispose);

    await controller.initialize();

    expect(registrar.callback, isNull);
    expect(controller.initialized, isTrue);
    expect(controller.busy, isFalse);
    expect(controller.errorMessage, 'Quick Paste could not be prepared.');
  });

  test('false open reaches the owning controller error state', () async {
    messenger.setMockMethodCallHandler(
      channel,
      (call) async => call.method != 'open',
    );
    final registrar = _Registrar();
    final controller = QuickPasteSettingsController(
      store: MemoryQuickPastePreferencesStore(),
      registrar: registrar,
      windowHost: host,
    );
    addTearDown(controller.dispose);
    await controller.initialize();

    await registrar.callback!();

    expect(controller.errorMessage, 'Quick Paste could not be opened.');
  });
}

class _Registrar implements DesktopShortcutRegistrar {
  Future<void> Function()? callback;

  @override
  Future<void> register(
    DesktopShortcut shortcut,
    Future<void> Function() callback,
  ) async {
    this.callback = callback;
  }

  @override
  Future<void> unregister() async {
    callback = null;
  }
}
