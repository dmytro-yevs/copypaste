import 'package:flutter_test/flutter_test.dart';
import 'package:flutter/services.dart';

import 'dart:async';

import 'package:copypaste_flutter/platform/android/android_shizuku_state.dart';
import 'package:copypaste_flutter/features/modules/controller/sms_access_setup_controller.dart';
import 'package:copypaste_flutter/features/modules/view/sms_access_setup_drawer.dart';
import 'package:copypaste_flutter/shared/android_access_setup.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/app/theme/app_tokens.dart';
import 'package:copypaste_flutter/features/modules/controller/modules_controller.dart';
import 'package:copypaste_flutter/features/modules/models/module_models.dart';
import 'package:copypaste_flutter/features/modules/repository/module_access_repository.dart';
import 'package:copypaste_flutter/features/modules/models/module_marketplace_models.dart';
import 'package:copypaste_flutter/features/modules/view/modules_settings_view.dart';

import 'modules_test_support.dart';

const sms = InstalledModule(
  id: 'copypaste.sms-codes',
  title: 'SMS Codes',
  description: 'Copy SMS codes.',
  version: '0.1.0',
  enabled: false,
  sizeBytes: 100,
  commands: [],
  preferenceFields: [],
  preferences: {},
  events: [ModuleEventKind.smsReceived],
);

class Access implements ModuleAccessRepository {
  bool granted = false;
  bool starts = true;
  bool notifications = true;
  bool supported = true;
  bool installed = true;
  bool running = true;
  bool permission = false;
  bool refusesGrants = false;
  bool refusesNotifications = false;
  int grants = 0;
  int opens = 0;
  int reads = 0;
  bool failsRead = false;
  Completer<SmsModuleAccessState>? pendingRead;
  int synchronizations = 0;
  @override
  Future<SmsModuleAccessState> smsState() async {
    reads++;
    if (failsRead) throw const ModulesException('State unavailable.');
    final pending = pendingRead;
    if (pending != null) {
      pendingRead = null;
      return pending.future;
    }
    return SmsModuleAccessState(
      smsGranted: granted,
      notificationGranted: notifications,
      shizuku: AndroidShizukuState(
        supported: supported,
        installed: installed,
        running: running,
        permission: permission,
      ),
      adbCommands: 'adb shell test\nadb shell otp',
    );
  }

  @override
  Future<bool> openShizuku() async {
    opens++;
    return true;
  }

  @override
  Future<SmsModuleAccessState> requestSmsNotifications() async {
    notifications = !refusesNotifications;
    return smsState();
  }

  @override
  Future<SmsModuleAccessState> configureSms() async {
    grants++;
    granted = !refusesGrants;
    return smsState();
  }

  @override
  Future<bool> synchronize() async {
    synchronizations++;
    return starts;
  }
}

void main() {
  for (final (platform, width, height, scale) in [
    (TargetPlatform.android, 320.0, 640.0, 2.0),
    (TargetPlatform.android, 640.0, 360.0, 1.0),
    (TargetPlatform.macOS, 1000.0, 700.0, 1.0),
    (TargetPlatform.windows, 1000.0, 700.0, 1.0),
  ]) {
    testWidgets(
      'SMS setup drawer and enable work on $platform at width $width',
      (tester) async {
        await tester.binding.setSurfaceSize(Size(width, height));
        addTearDown(() => tester.binding.setSurfaceSize(null));
        final repository = MemoryModulesRepository()..modules = [sms];
        final controller = ModulesController(
          repository: repository,
          marketplace: MemoryModuleMarketplace(),
          access: Access(),
        );
        addTearDown(controller.dispose);
        await controller.initialize();
        controller.selectSection(ModulesSection.installed);
        await tester.pumpWidget(
          ShadcnApp(
            theme: AppTheme.light.copyWith(platform: () => platform),
            builder: (context, child) => AppTheme.builder(
              context,
              MediaQuery(
                data: MediaQuery.of(context)
                    .copyWith(textScaler: TextScaler.linear(scale)),
                child: child!,
              ),
            ),
            home: Scaffold(
              child: SingleChildScrollView(
                child: Padding(
                  padding: const EdgeInsets.all(AppSpacing.lg),
                  child: ModulesSettingsView(controller: controller),
                ),
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();
        final settings = find.byKey(
          const ValueKey('module-settings-copypaste.sms-codes'),
        );
        await tester.ensureVisible(settings);
        await tester.pumpAndSettle();
        await tester.tap(settings);
        await tester.pumpAndSettle();
        await tester.ensureVisible(
          find.widgetWithText(Button, 'Set up SMS access'),
        );
        await tester.pumpAndSettle();
        await tester.tap(find.widgetWithText(Button, 'Set up SMS access'));
        await tester.pumpAndSettle();
        final drawer = find.byKey(const ValueKey('sms-access-setup-drawer'));
        expect(drawer, findsOneWidget);
        expect(tester.getSize(drawer).width, closeTo(width, 2));
        expect(
          tester.getSize(drawer).height,
          lessThanOrEqualTo(
            MediaQuery.sizeOf(tester.element(drawer)).height *
                AppOverlaySize.drawerHeightFactor,
          ),
        );
        expect(tester.getTopLeft(drawer).dy, greaterThanOrEqualTo(0));
        expect(tester.getBottomRight(drawer).dy, lessThanOrEqualTo(height));
        final title = find.byKey(const ValueKey('sms-access-setup-title'));
        final icon = find.byKey(const ValueKey('sms-access-setup-icon'));
        expect(
          tester.getCenter(icon).dy,
          closeTo(tester.getCenter(title).dy, 0.01),
        );
        final headerPosition = tester.getTopLeft(title);
        final done = find.byKey(const ValueKey('sms-access-setup-done'));
        expect(tester.getBottomRight(done).dy, lessThan(height));
        expect(find.byType(AndroidAccessSetup), findsOneWidget);
        expect(find.byType(SelectableText), findsNothing);
        await tester.ensureVisible(find.text('ADB'));
        await tester.pumpAndSettle();
        expect(tester.getTopLeft(title), headerPosition);
        expect(tester.getBottomRight(done).dy, lessThan(height));
        final methodTabs = find.descendant(
          of: find.byType(AndroidAccessSetup),
          matching: find.byType(TabItem),
        );
        final shizukuTab = methodTabs.first;
        final adbTab = methodTabs.last;
        expect(
          tester.getSize(shizukuTab).width,
          closeTo(tester.getSize(adbTab).width, 0.01),
        );
        expect(
          tester.getCenter(find.text('Shizuku')).dy,
          closeTo(tester.getCenter(find.text('ADB')).dy, 0.01),
        );
        expect(
          tester.getSize(find.text('Shizuku')).height,
          closeTo(tester.getSize(find.text('ADB')).height, 0.01),
        );
        await tester.tap(find.text('ADB'));
        await tester.pumpAndSettle();
        expect(find.text('adb shell test\nadb shell otp'), findsOneWidget);
        await tester.ensureVisible(find.text('Shizuku'));
        await tester.pumpAndSettle();
        await tester.tap(find.text('Shizuku'));
        await tester.pumpAndSettle();
        await tester.ensureVisible(
          find.widgetWithText(Button, 'Allow CopyPaste'),
        );
        await tester.pumpAndSettle();
        await tester.tap(find.widgetWithText(Button, 'Allow CopyPaste'));
        await tester.pumpAndSettle();
        expect(
          find.text(
            'Access is ready. Enable SMS Codes to start copying new codes.',
          ),
          findsOneWidget,
        );
        await tester.tap(done);
        await tester.pumpAndSettle();
        expect(drawer, findsNothing);
        expect(
          find.byKey(
            const ValueKey('module-settings-drawer-copypaste.sms-codes'),
          ),
          findsOneWidget,
        );
        await tester.ensureVisible(find.byType(Switch));
        await tester.pumpAndSettle();
        await tester.tap(find.byType(Switch));
        await tester.pumpAndSettle();
        expect(repository.calls, ['enabled:true']);
        expect(tester.takeException(), isNull);
      },
    );
  }
  for (final state in ['missing', 'stopped', 'unsupported', 'ready']) {
    testWidgets('SMS setup handles Shizuku $state', (tester) async {
      final access = Access()
        ..installed = state != 'missing'
        ..running = state != 'missing' && state != 'stopped'
        ..supported = state != 'unsupported'
        ..granted = state == 'ready';
      final setup = SmsAccessSetupController(access: access);
      addTearDown(setup.dispose);
      await tester.pumpWidget(
        ShadcnApp(
          theme: AppTheme.light,
          builder: AppTheme.builder,
          home: SmsAccessSetupDrawer(controller: setup),
        ),
      );
      await tester.pumpAndSettle();
      if (state == 'missing' || state == 'stopped') {
        final label = state == 'missing' ? 'Get Shizuku' : 'Open Shizuku';
        await tester.ensureVisible(find.widgetWithText(Button, label));
        await tester.pumpAndSettle();
        await tester.tap(find.widgetWithText(Button, label));
        await tester.pumpAndSettle();
        expect(access.opens, 1);
        expect(access.grants, 0);
      } else if (state == 'unsupported') {
        expect(find.text('Use the ADB tab on this device.'), findsOneWidget);
      } else {
        expect(find.text('One-time access applied'), findsOneWidget);
        expect(find.text('Allow CopyPaste'), findsNothing);
      }
      await tester.pumpWidget(const SizedBox.shrink());
    });
  }

  testWidgets('ADB grants update live and commands copy as one block', (
    tester,
  ) async {
    String? copied;
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      SystemChannels.platform,
      (call) async {
        if (call.method == 'Clipboard.setData') {
          copied = (call.arguments as Map)['text'] as String;
        }
        return null;
      },
    );
    addTearDown(
      () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        SystemChannels.platform,
        null,
      ),
    );
    final access = Access();
    final setup = SmsAccessSetupController(access: access);
    addTearDown(setup.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        builder: AppTheme.builder,
        home: SmsAccessSetupDrawer(controller: setup),
      ),
    );
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.text('ADB'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('ADB'));
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.byIcon(LucideIcons.copy));
    await tester.pumpAndSettle();
    await tester.tap(find.byIcon(LucideIcons.copy));
    await tester.pumpAndSettle();
    expect(copied, 'adb shell test\nadb shell otp');
    expect(find.byIcon(LucideIcons.copyCheck), findsOneWidget);
    expect(setup.state?.granted, isFalse);
    access.granted = true;
    await tester.pump(const Duration(seconds: 1));
    await tester.pumpAndSettle();
    expect(setup.state?.granted, isTrue);
    expect(access.grants, 0);
    await tester.pumpWidget(const SizedBox.shrink());
    final reads = access.reads;
    await tester.pump(const Duration(seconds: 2));
    expect(access.reads, reads);
  });

  testWidgets(
    'SMS notifications and access fit small screens with large text',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(320, 640));
      tester.platformDispatcher.textScaleFactorTestValue = 2;
      addTearDown(() => tester.binding.setSurfaceSize(null));
      addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
      final access = Access()..notifications = false;
      final setup = SmsAccessSetupController(access: access);
      addTearDown(setup.dispose);
      await tester.pumpWidget(
        ShadcnApp(
          theme: AppTheme.light,
          builder: AppTheme.builder,
          home: SmsAccessSetupDrawer(controller: setup),
        ),
      );
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      await tester.ensureVisible(find.widgetWithText(Button, 'Allow'));
      await tester.ensureVisible(find.widgetWithText(Button, 'Allow'));
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(Button, 'Allow'));
      await tester.pumpAndSettle();
      expect(setup.state?.notificationGranted, isTrue);
      await tester.ensureVisible(find.text('ADB'));
      await tester.ensureVisible(find.text('ADB'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('ADB'));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    },
  );

  testWidgets('denied SMS grants never show ready or enable the module', (
    tester,
  ) async {
    final access = Access()..refusesGrants = true;
    final setup = SmsAccessSetupController(access: access);
    addTearDown(setup.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        builder: AppTheme.builder,
        home: SmsAccessSetupDrawer(controller: setup),
      ),
    );
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.widgetWithText(Button, 'Allow CopyPaste'));
    await tester.pumpAndSettle();
    await tester.tap(find.widgetWithText(Button, 'Allow CopyPaste'));
    await tester.pumpAndSettle();
    expect(find.text('SMS access was not granted.'), findsOneWidget);
    expect(find.text('One-time access applied'), findsNothing);
    expect(setup.state?.granted, isFalse);
    final repository = MemoryModulesRepository()..modules = [sms];
    final modules = ModulesController(
      repository: repository,
      marketplace: MemoryModuleMarketplace(),
      access: access,
    );
    addTearDown(modules.dispose);
    await modules.initialize();
    await modules.setEnabled(sms.id, true);
    expect(repository.calls, isEmpty);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('authorized Shizuku applies SMS grants once', (tester) async {
    final access = Access()
      ..permission = true
      ..refusesGrants = true;
    final setup = SmsAccessSetupController(access: access);
    addTearDown(setup.dispose);
    await tester.pumpWidget(
      ShadcnApp(home: SmsAccessSetupDrawer(controller: setup)),
    );
    await tester.pumpAndSettle();
    expect(access.grants, 1);
    await tester.pump(const Duration(seconds: 3));
    await tester.pumpAndSettle();
    expect(access.grants, 1);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('monitoring stops while paused and refreshes on resume', (
    tester,
  ) async {
    final access = Access();
    final setup = SmsAccessSetupController(access: access);
    addTearDown(setup.dispose);
    setup.setMonitoring(true);
    await tester.pump();
    setup.setMonitoring(false);
    final reads = access.reads;
    access.granted = true;
    await tester.pump(const Duration(seconds: 3));
    expect(access.reads, reads);
    expect(setup.state?.granted, isFalse);
    setup.setMonitoring(true);
    await tester.pump();
    expect(setup.state?.granted, isTrue);
    setup.setMonitoring(false);
  });

  test(
    'failed state reads recover without retaining a stale setup error',
    () async {
      final access = Access()..failsRead = true;
      final setup = SmsAccessSetupController(access: access);
      addTearDown(setup.dispose);
      await setup.refresh();
      expect(setup.errorMessage, contains('could not be verified'));
      access.failsRead = false;
      await setup.refresh();
      expect(setup.errorMessage, isNull);
      expect(setup.state?.granted, isFalse);
    },
  );

  test('a slow refresh cannot overwrite newly applied grants', () async {
    final access = Access();
    final before = await access.smsState();
    final pending = Completer<SmsModuleAccessState>();
    access.pendingRead = pending;
    final setup = SmsAccessSetupController(access: access);
    addTearDown(setup.dispose);
    final reading = setup.refresh();
    await setup.applyAccess();
    expect(setup.state?.granted, isTrue);
    pending.complete(before);
    await reading;
    expect(setup.state?.granted, isTrue);
  });

  test('refused notifications keep SMS access incomplete', () async {
    final access = Access()
      ..granted = true
      ..notifications = false
      ..refusesNotifications = true;
    final setup = SmsAccessSetupController(access: access);
    addTearDown(setup.dispose);
    await setup.requestNotifications();
    expect(setup.state?.granted, isFalse);
    expect(setup.errorMessage, contains('Allow notifications'));
  });

  test('requires verified SMS access before enabling and synchronizes disable and removal', () async {
    final repository = MemoryModulesRepository()..modules = [sms];
    final access = Access();
    final controller = ModulesController(
      repository: repository,
      marketplace: MemoryModuleMarketplace(),
      access: access,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await controller.setEnabled(sms.id, true);
    expect(repository.calls, isEmpty);
    expect(controller.errorMessage, contains('Set up SMS access'));
    final setup = controller.smsAccessSetup();
    addTearDown(setup.dispose);
    await setup.applyAccess();
    await controller.setEnabled(sms.id, true);
    expect(controller.modules.single.enabled, isTrue);
    await controller.setEnabled(sms.id, false);
    expect(controller.modules.single.enabled, isFalse);
    await controller.remove(sms.id);
    expect(controller.modules, isEmpty);
    expect(access.synchronizations, greaterThanOrEqualTo(4));
  });
  test('SMS grants without notifications cannot enable monitoring', () async {
    final repository = MemoryModulesRepository()..modules = [sms];
    final access = Access()
      ..granted = true
      ..notifications = false;
    final modules = ModulesController(
      repository: repository,
      marketplace: MemoryModuleMarketplace(),
      access: access,
    );
    addTearDown(modules.dispose);
    await modules.initialize();
    await modules.setEnabled(sms.id, true);
    expect(repository.calls, isEmpty);
    expect(modules.errorMessage, contains('Set up SMS access'));
  });

  test('failed monitoring startup rolls back enabled state', () async {
    final repository = MemoryModulesRepository()..modules = [sms];
    final access = Access()..granted = true;
    final controller = ModulesController(
      repository: repository,
      marketplace: MemoryModuleMarketplace(),
      access: access,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    access.starts = false;
    await controller.setEnabled(sms.id, true);
    expect(repository.calls, ['enabled:true', 'enabled:false']);
    expect(controller.modules.single.enabled, isFalse);
    expect(controller.errorMessage, contains('could not be started'));
  });
}
