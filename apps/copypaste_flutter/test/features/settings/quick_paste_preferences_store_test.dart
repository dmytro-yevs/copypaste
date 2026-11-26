import 'dart:convert';

import 'package:copypaste_flutter/features/settings/repository/quick_paste_preferences_store.dart';
import 'package:copypaste_flutter/platform/desktop/global_shortcut.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';

void main() {
  test('saved Ctrl+C is replaced without resetting auto-paste', () async {
    const reserved = DesktopShortcut(
      key: PhysicalKeyboardKey.keyC,
      modifiers: [DesktopShortcutModifier.control],
    );
    final store = SharedPreferencesQuickPastePreferencesStore(
      preferences: _Preferences(
        jsonEncode({'autoPaste': false, 'shortcut': reserved.toJson()}),
      ),
    );
    final preferences = await store.read();
    expect(preferences.shortcut, DesktopShortcut.defaultForPlatform());
    expect(preferences.autoPaste, isFalse);
  });

  test('password field masking is on until it is saved off', () async {
    final store = SharedPreferencesQuickPastePreferencesStore(
      preferences: _Preferences(
        jsonEncode({
          'autoPaste': true,
          'shortcut': DesktopShortcut.defaultForPlatform().toJson(),
        }),
      ),
    );
    final saved = await store.read();
    expect(saved.maskPasswordFields, isTrue);

    await store.write(saved.copyWith(maskPasswordFields: false));

    expect((await store.read()).maskPasswordFields, isFalse);
  });
}

class _Preferences extends Fake implements SharedPreferencesAsync {
  _Preferences(String encoded) : _saved = [encoded];
  final List<String> _saved;

  @override
  Future<String?> getString(String key) async => _saved.last;

  @override
  Future<void> setString(String key, String value) async => _saved.add(value);
}
