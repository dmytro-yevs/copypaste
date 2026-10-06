import 'dart:async';

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

  for (final platform in [TargetPlatform.macOS, TargetPlatform.windows]) {
    test('$platform open requires native true', () async {
      debugDefaultTargetPlatformOverride = platform;
      await host.open();
      response = null;
      await expectLater(host.open(), throwsA(isA<PlatformException>()));
      response = false;
      await expectLater(host.open(), throwsA(isA<PlatformException>()));
      response = 'success';
      await expectLater(host.open(), throwsA(isA<TypeError>()));
    });
  }

  test('prepare requires confirmed native success', () async {
    response = null;
    await expectLater(host.prepare(), throwsA(isA<PlatformException>()));
  });

  test(
    'initialization registers the shortcut without preparing an engine',
    () async {
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

      expect(registrar.callback, isNotNull);
      expect(controller.initialized, isTrue);
      expect(controller.busy, isFalse);
      expect(controller.errorMessage, isNull);
    },
  );

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
  group('typed presentation context', () {
    const contextChannel = MethodChannel('test/quick_paste_context');
    late MethodChannelQuickPasteContextHost context;
    final calls = <MethodCall>[];
    setUp(() {
      calls.clear();
      context = MethodChannelQuickPasteContextHost(channel: contextChannel);
      messenger.setMockMethodCallHandler(contextChannel, (call) async {
        calls.add(call);
        return response;
      });
    });
    tearDown(() async {
      await context.dispose();
      messenger.setMockMethodCallHandler(contextChannel, null);
    });

    Future<Object?> invokeContext(String method, Object? arguments) async {
      final completion = Completer<Object?>();
      await messenger.handlePlatformMessage(
        contextChannel.name,
        const StandardMethodCodec().encodeMethodCall(
          MethodCall(method, arguments),
        ),
        (reply) {
          completion.complete(reply);
        },
      );
      return completion.future;
    }

    Future<Object?> opened(Object? arguments) =>
        invokeContext('opened', arguments);

    test('ready requires a native acknowledgement', () async {
      await context.signalReady();
      expect(calls.single.method, 'ready');
      response = false;
      await expectLater(
        context.signalReady(),
        throwsA(isA<PlatformException>()),
      );
    });

    test('shutdown acknowledges only after its cleanup completes', () async {
      final cleanup = Completer<void>();
      context.setShutdownHandler(() => cleanup.future);
      var acknowledged = false;
      final pending = invokeContext(
        'shutdown',
        null,
      ).then((_) => acknowledged = true);
      await Future<void>.delayed(Duration.zero);
      expect(acknowledged, isFalse);
      cleanup.complete();
      await pending;
      expect(acknowledged, isTrue);
    });

    test(
      'paste and conditional close send the originating native ID',
      () async {
        expect(await context.paste(presentationId: 41), isTrue);
        await context.close(presentationId: 41);
        expect(calls.map((call) => call.method), ['paste', 'close']);
        expect(
          calls.map((call) => call.arguments),
          everyElement({'presentationId': 41}),
        );
      },
    );

    for (final value in [
      false,
      null,
      'success',
      1,
      {'ok': true},
    ]) {
      test('paste rejects malformed or refused result $value', () async {
        response = value;
        expect(await context.paste(presentationId: 41), isFalse);
      });
    }
    for (final error in [
      PlatformException(code: 'closed'),
      MissingPluginException(),
    ]) {
      test('optional paste channel $error returns false', () async {
        messenger.setMockMethodCallHandler(contextChannel, (call) async {
          throw error;
        });
        expect(await context.paste(presentationId: 41), isFalse);
      });
    }
    test(
      'opened carries its positive identity before handler awaits',
      () async {
        int? received;
        final gate = Completer<void>();
        context.setOpenedHandler((id, {required bool inspectorVisible}) async {
          received = id;
          await gate.future;
        });
        final completion = opened({'presentationId': 42});
        await Future<void>.delayed(Duration.zero);
        expect(received, 42);
        gate.complete();
        await completion;
      },
    );
    for (final value in [
      null,
      {},
      {'presentationId': 0},
      {'presentationId': -1},
      {'presentationId': true},
      {'presentationId': 1.0},
      {'presentationId': '1'},
    ]) {
      test('invalid opened $value cannot invoke the handler', () async {
        var invoked = false;
        context.setOpenedHandler((_, {required bool inspectorVisible}) async {
          invoked = true;
        });
        await opened(value);
        expect(invoked, isFalse);
      });
    }
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
