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

  test('native Find routes to the shared History search action', () async {
    var finds = 0;
    host.setFindInHistoryHandler(() => finds += 1);
    final completion = Completer<void>();
    var acknowledged = false;
    await messenger.handlePlatformMessage(
      channel.name,
      const StandardMethodCodec().encodeMethodCall(
        const MethodCall('findInHistory'),
      ),
      (reply) {
        acknowledged =
            const StandardMethodCodec().decodeEnvelope(reply!) == true;
        completion.complete();
      },
    );
    await completion.future;
    expect(acknowledged, isTrue);
    expect(finds, 1);
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

  test(
    'tray presentation sends icon bounds through the existing host',
    () async {
      final calls = <MethodCall>[];
      messenger.setMockMethodCallHandler(channel, (call) async {
        calls.add(call);
        return true;
      });
      await openTrayQuickPaste(
        const Rect.fromLTWH(-1240, 8, 24, 24),
        channel: channel,
      );
      expect(calls.single.method, 'openFromTray');
      expect(calls.single.arguments, {
        'x': -1240.0,
        'y': 8.0,
        'width': 24.0,
        'height': 24.0,
      });
    },
  );

  test('tray presentation requires native acknowledgement', () async {
    response = false;
    await expectLater(
      openTrayQuickPaste(const Rect.fromLTWH(10, 0, 24, 24), channel: channel),
      throwsA(isA<PlatformException>()),
    );
  });

  test('invalid tray geometry never reaches the native host', () async {
    var invoked = false;
    messenger.setMockMethodCallHandler(channel, (call) async {
      invoked = true;
      return true;
    });
    for (final anchor in [Rect.zero, Rect.fromLTWH(double.nan, 0, 24, 24)]) {
      await expectLater(
        openTrayQuickPaste(anchor, channel: channel),
        throwsArgumentError,
      );
    }
    expect(invoked, isFalse);
  });

  for (final platform in [
    TargetPlatform.macOS,
    TargetPlatform.windows,
    TargetPlatform.linux,
  ]) {
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

    test('hidden awaits cleanup for its exact presentation identity', () async {
      int? received;
      var acknowledged = false;
      final gate = Completer<void>();
      context.setHiddenHandler((id) async {
        received = id;
        await gate.future;
      });
      final pending = invokeContext('hidden', {
        'presentationId': 42,
      }).then((_) => acknowledged = true);
      await Future<void>.delayed(Duration.zero);
      expect(received, 42);
      expect(acknowledged, isFalse);
      gate.complete();
      await pending;
      expect(acknowledged, isTrue);
    });

    test('reads only a valid child runtime endpoint', () async {
      response = '/tmp/copypaste.sock';
      expect(await context.runtimeEndpoint(), '/tmp/copypaste.sock');
      expect(calls.last.method, 'runtimeEndpoint');

      for (final value in [null, '', 'relative.sock', 1]) {
        response = value;
        await expectLater(
          context.runtimeEndpoint(),
          throwsA(
            isA<PlatformException>().having(
              (error) => error.code,
              'code',
              'runtime_endpoint_unavailable',
            ),
          ),
        );
      }
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

    test(
      'only a rendered presentation can request native reveal by its ID',
      () async {
        expect(await context.presentFrame(presentationId: 41), isTrue);
        expect(calls.single.method, 'present');
        expect(calls.single.arguments, {'presentationId': 41});
        response = false;
        expect(await context.presentFrame(presentationId: 42), isFalse);
      },
    );

    test(
      'inspector visibility carries explicit persistence separately',
      () async {
        await context.setInspectorVisible(
          presentationId: 41,
          visible: true,
          remember: false,
        );
        await context.setInspectorVisible(presentationId: 41, visible: false);
        expect(calls.map((call) => call.arguments).toList(), [
          {'presentationId': 41, 'visible': true, 'remember': false},
          {'presentationId': 41, 'visible': false, 'remember': true},
        ]);
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
      test('invalid hidden $value cannot invoke the handler', () async {
        var invoked = false;
        context.setHiddenHandler((_) async {
          invoked = true;
        });
        await invokeContext('hidden', value);
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
