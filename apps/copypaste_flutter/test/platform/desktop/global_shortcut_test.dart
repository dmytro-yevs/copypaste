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
}
