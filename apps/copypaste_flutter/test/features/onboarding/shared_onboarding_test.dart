import 'dart:async';

import 'package:copypaste_flutter/features/onboarding/controller/onboarding_controller.dart';
import 'package:copypaste_flutter/features/onboarding/controller/onboarding_setup.dart';
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
      final setup = _Setup();
      final controller = await _ready(setup);
      final pending = Completer<void>();
      setup.persist = () => pending.future;
      final first = controller.finish();
      expect(controller.busy, isTrue);
      setup.persist = () async => fail('duplicate persistence');
      expect(await controller.finish(), isFalse);
      pending.completeError(StateError('storage failed'));
      expect(await first, isFalse);
      expect(controller.complete, isFalse);
      expect(controller.errorMessage, isNotNull);
      setup.persist = () async {};
      expect(await controller.finish(), isTrue);
      expect(controller.complete, isTrue);
      expect(controller.errorMessage, isNull);
      controller.dispose();
    },
  );

  test(
    'late completion after disposal never completes the setup flow',
    () async {
      final setup = _Setup();
      final controller = await _ready(setup);
      final pending = Completer<void>();
      setup.persist = () => pending.future;
      final result = controller.finish();
      controller.dispose();
      pending.complete();
      expect(await result, isFalse);
      expect(controller.complete, isFalse);
    },
  );

  test(
    'visible setup refreshes on resume or follows the change stream',
    () async {
      final refreshing = _Setup();
      final flow = OnboardingController(setup: refreshing);
      addTearDown(flow.dispose);
      await flow.initialize();
      flow.setAttached(true);
      flow.setForeground(false);
      flow.setForeground(true);
      expect(refreshing.refreshes, 0, reason: 'Welcome is not the setup step.');
      await flow.showSetup();
      flow.setForeground(false);
      flow.setForeground(true);
      expect(refreshing.refreshes, 1);

      final streaming = _Setup(monitorsChanges: true);
      final monitored = OnboardingController(setup: streaming);
      addTearDown(monitored.dispose);
      await monitored.initialize();
      monitored.setAttached(true);
      await monitored.showSetup();
      expect(streaming.monitoring, isTrue);
      monitored.setForeground(false);
      expect(streaming.monitoring, isFalse);
      monitored.setForeground(true);
      expect(streaming.monitoring, isTrue);
      expect(streaming.refreshes, 0);
      monitored.showPreviousStep();
      expect(streaming.monitoring, isFalse);
    },
  );
}

Future<OnboardingController> _ready(_Setup setup) async {
  final controller = OnboardingController(setup: setup);
  await controller.initialize();
  await controller.showSetup();
  expect(await controller.continueFromSetup(), isTrue);
  expect(controller.step, OnboardingStep.ready);
  return controller;
}

class _Setup extends OnboardingSetup {
  _Setup({this.monitorsChanges = false});

  @override
  final bool monitorsChanges;
  Future<void> Function() persist = () async {};
  int refreshes = 0;
  bool monitoring = false;

  @override
  String get platformLabel => 'Test';

  @override
  Future<OnboardingResume> restore() async => OnboardingResume.welcome;

  @override
  Future<bool> apply() async => true;

  @override
  Future<void> persistCompletion() => persist();

  @override
  Future<void> refresh() async => refreshes++;

  @override
  void setMonitoring(bool enabled) => monitoring = enabled;
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
