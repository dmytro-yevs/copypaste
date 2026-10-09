import 'dart:async';

import 'package:copypaste_flutter/platform/desktop/global_shortcut.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hotkey_manager/hotkey_manager.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('dev.leanflutter.plugins/hotkey_manager');
  const events = MethodChannel('dev.leanflutter.plugins/hotkey_manager_event');
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
  const shortcut = DesktopShortcut(
    key: PhysicalKeyboardKey.f6,
    modifiers: [DesktopShortcutModifier.meta],
  );
  late HotKeyManagerDesktopShortcutRegistrar registrar;
  late List<String> calls;
  bool failRelease = false;
  bool failRegistration = false;

  setUp(() {
    calls = [];
    failRelease = false;
    failRegistration = false;
    messenger.setMockMethodCallHandler(events, (_) async => null);
    messenger.setMockMethodCallHandler(channel, (call) async {
      calls.add(call.method);
      if (call.method == 'unregister' && failRelease) {
        throw PlatformException(code: 'hotkey_unregistration_failed');
      }
      if (call.method == 'register' && failRegistration) {
        throw PlatformException(code: 'hotkey_registration_failed');
      }
      return true;
    });
    registrar = HotKeyManagerDesktopShortcutRegistrar();
  });

  tearDown(() async {
    failRelease = false;
    await registrar.unregister();
    messenger.setMockMethodCallHandler(channel, null);
  });

  test(
    'failed release retains callback ownership and blocks replacement',
    () async {
      await registrar.register(shortcut, () async {});
      final original = hotKeyManager.registeredHotKeyList.single;
      failRelease = true;
      await expectLater(
        registrar.unregister(),
        throwsA(isA<PlatformException>()),
      );
      await expectLater(
        registrar.register(shortcut, () async {}),
        throwsA(isA<PlatformException>()),
      );
      expect(calls, ['register', 'unregister', 'unregister']);
      expect(hotKeyManager.registeredHotKeyList.single, same(original));
      failRelease = false;
      await registrar.unregister();
      await registrar.unregister();
      expect(calls, ['register', 'unregister', 'unregister', 'unregister']);
      expect(hotKeyManager.registeredHotKeyList, isEmpty);
    },
  );

  test(
    'registration failure adds no callback or native release obligation',
    () async {
      failRegistration = true;
      await expectLater(
        registrar.register(shortcut, () async {}),
        throwsA(isA<PlatformException>()),
      );
      await registrar.unregister();
      expect(calls, ['register']);
      expect(hotKeyManager.registeredHotKeyList, isEmpty);
    },
  );

  group('Linux portal registrar', () {
    const linuxChannel = MethodChannel('test/linux_shortcuts');
    late LinuxPortalDesktopShortcutRegistrar linuxRegistrar;
    final linuxCalls = <MethodCall>[];

    setUp(() {
      linuxCalls.clear();
      messenger.setMockMethodCallHandler(linuxChannel, (call) async {
        linuxCalls.add(call);
        return switch (call.method) {
          'isSupported' => true,
          'register' => {
            'registered': true,
            'triggerDescription': 'Ctrl+Shift+C',
          },
          'unregister' => true,
          _ => null,
        };
      });
      linuxRegistrar = LinuxPortalDesktopShortcutRegistrar(
        channel: linuxChannel,
      );
    });

    tearDown(() async {
      await linuxRegistrar.unregister();
      messenger.setMockMethodCallHandler(linuxChannel, null);
    });

    test(
      'uses the portal contract and runs only matching activations',
      () async {
        var activations = 0;
        await linuxRegistrar.register(
          const DesktopShortcut(
            key: PhysicalKeyboardKey.keyC,
            modifiers: [
              DesktopShortcutModifier.control,
              DesktopShortcutModifier.shift,
            ],
          ),
          () async => activations += 1,
        );

        expect(linuxCalls.map((call) => call.method), [
          'isSupported',
          'register',
        ]);
        expect(linuxCalls.last.arguments, {
          'id': 'copypaste.quick-paste',
          'description': 'Open Quick Paste',
          'preferredTrigger': 'CTRL+SHIFT+C',
          'usage': PhysicalKeyboardKey.keyC.usbHidUsage,
          'modifiers': ['control', 'shift'],
        });
        expect(linuxRegistrar.registeredTriggerDescription, 'Ctrl+Shift+C');

        Future<Object?> activate(Object? arguments) async {
          final completion = Completer<Object?>();
          await messenger.handlePlatformMessage(
            linuxChannel.name,
            const StandardMethodCodec().encodeMethodCall(
              MethodCall('activated', arguments),
            ),
            (reply) => completion.complete(
              reply == null
                  ? null
                  : const StandardMethodCodec().decodeEnvelope(reply),
            ),
          );
          return completion.future;
        }

        expect(await activate({'id': 'other'}), isFalse);
        expect(await activate({'id': 'copypaste.quick-paste'}), isTrue);
        await Future<void>.delayed(Duration.zero);
        expect(activations, 1);
      },
    );

    test('rejects unavailable and refused portal registration', () async {
      messenger.setMockMethodCallHandler(linuxChannel, (call) async {
        if (call.method == 'isSupported') return false;
        return true;
      });
      await expectLater(
        linuxRegistrar.register(shortcut, () async {}),
        throwsA(isA<PlatformException>()),
      );
      expect(linuxCalls, isEmpty);

      messenger.setMockMethodCallHandler(linuxChannel, (call) async {
        if (call.method == 'isSupported') return true;
        if (call.method == 'register') return {'registered': false};
        return true;
      });
      await expectLater(
        linuxRegistrar.register(shortcut, () async {}),
        throwsA(isA<PlatformException>()),
      );
    });

    test('encodes non-letter keys without whitespace or layout text', () {
      expect(
        const DesktopShortcut(
          key: PhysicalKeyboardKey.arrowDown,
          modifiers: [DesktopShortcutModifier.control],
        ).linuxPreferredTrigger,
        'CTRL+DOWN',
      );
      expect(
        const DesktopShortcut(
          key: PhysicalKeyboardKey.space,
          modifiers: [DesktopShortcutModifier.alt],
        ).linuxPreferredTrigger,
        'ALT+SPACE',
      );
      expect(
        const DesktopShortcut(
          key: PhysicalKeyboardKey.semicolon,
          modifiers: [DesktopShortcutModifier.shift],
        ).linuxPreferredTrigger,
        'SHIFT+SEMICOLON',
      );
      expect(
        const DesktopShortcut(
          key: PhysicalKeyboardKey.f6,
          modifiers: [DesktopShortcutModifier.meta],
        ).linuxPreferredTrigger,
        'META+F6',
      );
      expect(
        const DesktopShortcut(
          key: PhysicalKeyboardKey.audioVolumeUp,
          modifiers: [DesktopShortcutModifier.alt],
        ).linuxPreferredTrigger,
        'ALT+HID_70080',
      );
    });
  });
}
