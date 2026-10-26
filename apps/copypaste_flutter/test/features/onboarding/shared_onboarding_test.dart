import 'dart:async';

import 'package:copypaste_flutter/features/onboarding/controller/onboarding_controller.dart';
import 'package:copypaste_flutter/features/onboarding/repository/onboarding_store.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';

void main() {
  test(
    'versioned store preserves every existing desktop preference key',
    () async {
      final preferences = _Preferences();
      for (final platform in OnboardingPlatform.values) {
        final key = 'onboarding.${platform.name}.version';
        final store = SharedPreferencesOnboardingStore(
          platform: platform,
          preferences: preferences,
        );
        expect(await store.isComplete(), isFalse);
        await store.markComplete();
        expect(preferences.values[key], 1);
        expect(await store.isComplete(), isTrue);
        final next = SharedPreferencesOnboardingStore(
          platform: platform,
          currentVersion: 2,
          preferences: preferences,
        );
        expect(await next.isComplete(), isFalse);
        await next.markComplete();
        expect(preferences.values[key], 2);
      }
      expect(
        preferences.values.keys,
        unorderedEquals([
          'onboarding.macos.version',
          'onboarding.windows.version',
          'onboarding.linux.version',
        ]),
      );
    },
  );

  test(
    'completion serializes persistence and retries only after an actual failure',
    () async {
      final controller = _Controller();
      final pending = Completer<void>();
      final first = controller.finish(() => pending.future);
      expect(controller.busy, isTrue);
      expect(
        await controller.finish(() async => fail('duplicate persistence')),
        isFalse,
      );
      pending.completeError(StateError('storage failed'));
      expect(await first, isFalse);
      expect(controller.complete, isFalse);
      expect(controller.errorMessage, isNotNull);
      expect(await controller.finish(() async {}), isTrue);
      expect(controller.complete, isTrue);
      expect(controller.errorMessage, isNull);
      controller.dispose();
    },
  );

  test(
    'late completion after disposal never completes the setup flow',
    () async {
      final controller = _Controller();
      final pending = Completer<void>();
      final result = controller.finish(() => pending.future);
      controller.dispose();
      pending.complete();
      expect(await result, isFalse);
      expect(controller.complete, isFalse);
    },
  );
}

class _Controller extends OnboardingController {
  Future<bool> finish(Future<void> Function() persist) =>
      finishOnboarding(canFinish: true, persist: persist);
}

class _Preferences implements SharedPreferencesAsync {
  final values = <String, int>{};
  @override
  Future<int?> getInt(String key) async => values[key];
  @override
  Future<void> setInt(String key, int value) async {
    values[key] = value;
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}
