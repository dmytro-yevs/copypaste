import 'dart:async';

import 'package:copypaste_flutter/platform/desktop/global_shortcut.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hotkey_manager/hotkey_manager.dart';

const _id = DesktopShortcutId.quickPaste;
const _description = 'Open Quick Paste';

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
    await registrar.unregister(_id);
    messenger.setMockMethodCallHandler(channel, null);
  });

  test('system copy is never reserved as a global shortcut', () async {
    const copy = DesktopShortcut(
      key: PhysicalKeyboardKey.keyC,
      modifiers: [DesktopShortcutModifier.control],
    );
    expect(copy.isValid, isFalse);
    expect(
      () => DesktopShortcut.fromJson(copy.toJson()),
      throwsFormatException,
    );
    await expectLater(
      registrar.register(_id, copy, () async {}, description: _description),
      throwsArgumentError,
    );
    expect(calls, isEmpty);
    const quickPaste = DesktopShortcut(
      key: PhysicalKeyboardKey.keyC,
      modifiers: [
        DesktopShortcutModifier.control,
        DesktopShortcutModifier.shift,
      ],
    );
    expect(quickPaste.isValid, isTrue);
    await registrar.register(
      _id,
      quickPaste,
      () async {},
      description: _description,
    );
    expect(calls, ['register']);
  });

  test(
    'failed release retains callback ownership and blocks replacement',
    () async {
      await registrar.register(
        _id,
        shortcut,
        () async {},
        description: _description,
      );
      final original = hotKeyManager.registeredHotKeyList.single;
      failRelease = true;
      await expectLater(
        registrar.unregister(_id),
        throwsA(isA<PlatformException>()),
      );
      await expectLater(
        registrar.register(
          _id,
          shortcut,
          () async {},
          description: _description,
        ),
        throwsA(isA<PlatformException>()),
      );
      expect(calls, ['register', 'unregister', 'unregister']);
      expect(hotKeyManager.registeredHotKeyList.single, same(original));
      failRelease = false;
      await registrar.unregister(_id);
      await registrar.unregister(_id);
      expect(calls, ['register', 'unregister', 'unregister', 'unregister']);
      expect(hotKeyManager.registeredHotKeyList, isEmpty);
    },
  );

  test(
    'registration failure adds no callback or native release obligation',
    () async {
      failRegistration = true;
      await expectLater(
        registrar.register(
          _id,
          shortcut,
          () async {},
          description: _description,
        ),
        throwsA(isA<PlatformException>()),
      );
      await registrar.unregister(_id);
      expect(calls, ['register']);
      expect(hotKeyManager.registeredHotKeyList, isEmpty);
    },
  );

  test('ids hold separate bindings and never share one combination', () async {
    const other = DesktopShortcut(
      key: PhysicalKeyboardKey.f7,
      modifiers: [DesktopShortcutModifier.meta],
    );
    await registrar.register(
      _id,
      shortcut,
      () async {},
      description: _description,
    );
    await registrar.register(
      'copypaste.paste-next',
      other,
      () async {},
      description: 'Paste next',
    );
    expect(hotKeyManager.registeredHotKeyList.map((key) => key.identifier), [
      _id,
      'copypaste.paste-next',
    ]);
    await expectLater(
      registrar.register(
        'copypaste.paste-next',
        shortcut,
        () async {},
        description: 'Paste next',
      ),
      throwsA(
        isA<DesktopShortcutConflict>().having(
          (conflict) => conflict.heldBy,
          'heldBy',
          _id,
        ),
      ),
    );
    expect(hotKeyManager.registeredHotKeyList, hasLength(2));
    await registrar.unregister('copypaste.paste-next');
    expect(hotKeyManager.registeredHotKeyList.single.identifier, _id);
  });

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
      await linuxRegistrar.unregister(_id);
      messenger.setMockMethodCallHandler(linuxChannel, null);
    });

    test(
      'uses the portal contract and runs only matching activations',
      () async {
        var activations = 0;
        await linuxRegistrar.register(
          _id,
          const DesktopShortcut(
            key: PhysicalKeyboardKey.keyC,
            modifiers: [
              DesktopShortcutModifier.control,
              DesktopShortcutModifier.shift,
            ],
          ),
          () async => activations += 1,
          description: _description,
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
        expect(
          linuxRegistrar.registeredTriggerDescription(_id),
          'Ctrl+Shift+C',
        );

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
        linuxRegistrar.register(
          _id,
          shortcut,
          () async {},
          description: _description,
        ),
        throwsA(isA<PlatformException>()),
      );
      expect(linuxCalls, isEmpty);

      messenger.setMockMethodCallHandler(linuxChannel, (call) async {
        if (call.method == 'isSupported') return true;
        if (call.method == 'register') return {'registered': false};
        return true;
      });
      await expectLater(
        linuxRegistrar.register(
          _id,
          shortcut,
          () async {},
          description: _description,
        ),
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
