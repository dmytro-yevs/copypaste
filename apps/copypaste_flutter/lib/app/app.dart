import 'dart:async';

import '../features/modules/controller/module_prompt_controller.dart';
import '../features/modules/controller/modules_controller.dart';
import '../features/modules/view/module_prompt_presenter.dart';

import '../features/transfers/file_operations_view.dart';
import '../features/notifications/notifications_controller.dart';
import '../features/notifications/notifications_view.dart';

import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../features/devices/devices.dart';
import '../features/paste_queue/view/paste_queue_header_action.dart';
import '../l10n/l10n.dart';
import 'localization/app_language.dart';
import '../features/history/controller/history_controller.dart';
import '../features/history/view/history_screen.dart';
import '../features/settings/controller/quick_paste_settings_controller.dart';
import '../features/settings/controller/settings_controller.dart';
import '../features/settings/controller/launch_at_login_controller.dart';
import '../features/settings/controller/storage_maintenance_controller.dart';
import 'device_preferences.dart';
import 'window_material_controller.dart';
import '../features/settings/view/capture_header_action.dart';
import '../features/settings/view/sync_header_action.dart';
import '../features/settings/view/storage_header_action.dart';
import '../features/settings/controller/settings_navigation_state.dart';
import '../features/settings/view/settings_screen.dart';
import '../features/update/controller/app_update_controller.dart';
import '../platform/desktop/desktop_window_controller.dart';
import '../platform/desktop/desktop_shortcuts.dart';
import '../shared/failure_toasts.dart';
import '../shared/state_view.dart';
import 'app_features.dart';
import 'navigation/navigation.dart';
import 'shell/shell.dart';
import 'theme/app_appearance.dart';
import 'theme/app_theme.dart';
import 'theme/app_tokens.dart';
import 'theme/app_window_material.dart';

class CopyPasteApp extends StatelessWidget {
  const CopyPasteApp({
    super.key,
    required this.navigation,
    this.historyController,
    this.devicesController,
    this.features,
    this.notifications,
    this.quickPasteSettings,
    this.settingsController,
    this.launchAtLogin,
    this.storage,
    this.devicePreferences,
    this.appearance,
    this.windowMaterial,
    this.modulesController,
    this.modulePrompts,
    this.appUpdateController,
    this.desktopWindow,
    this.runtimeStarting = false,
    this.runtimeFailureMessage,
    this.onRetryRuntime,
    this.onOpenAndroidCaptureSetup,
    this.onShowWelcomeTour,
    this.navigatorKey,
    this.language,
  });

  final AppNavigationController navigation;
  final HistoryController? historyController;
  final DevicesController? devicesController;
  final AppFeatures? features;
  final NotificationsController? notifications;
  final QuickPasteSettingsController? quickPasteSettings;
  final SettingsController? settingsController;
  final LaunchAtLoginController? launchAtLogin;
  final StorageMaintenanceController? storage;
  final DevicePreferences? devicePreferences;

  /// The theme mode and accent; the defaults without one.
  final AppAppearance? appearance;
  final WindowMaterialController? windowMaterial;
  final ModulesController? modulesController;
  final ModulePromptController? modulePrompts;
  final AppUpdateController? appUpdateController;
  final DesktopWindowController? desktopWindow;

  /// Whether the runtime is coming up, so its destinations are loading.
  final bool runtimeStarting;

  /// Why the runtime did not start; null unless it failed.
  final String? runtimeFailureMessage;
  final Future<void> Function()? onRetryRuntime;
  final Future<void> Function()? onOpenAndroidCaptureSetup;

  /// Starts the first-run tour again; null where there is no onboarding.
  final Future<void> Function()? onShowWelcomeTour;
  final GlobalKey<NavigatorState>? navigatorKey;
  final AppLanguageController? language;

  Widget _withoutRuntime(BuildContext context, String title) => runtimeStarting
      ? StateView.loading(message: context.l10n.appRuntimeWaiting)
      : StateView.error(
          title: title,
          message: runtimeFailureMessage,
          onRetry: onRetryRuntime,
        );

  @override
  Widget build(BuildContext context) {
    return AppTheme.withAppearance(
      appearance,
      (themeMode, light, dark) => ShadcnApp(
        navigatorKey: navigatorKey,
        title: 'CopyPaste',
        // The debug ribbon covers the header's end controls.
        debugShowCheckedModeBanner: false,
        theme: light,
        darkTheme: dark,
        themeMode: themeMode,
        builder: (context, child) {
          final content = AppTheme.builder(
            context,
            CallbackShortcuts(
              bindings: desktopWindowBindings(
                onClose: desktopWindow?.handleCloseRequested,
                onQuit: desktopWindow?.quit,
                onToggleFullScreen: desktopWindow?.toggleFullScreen,
              ),
              child: Shortcuts(
                shortcuts: desktopTextEditingShortcuts,
                child: child ?? const SizedBox.shrink(),
              ),
            ),
          );
          final material = windowMaterial;
          if (material == null) return content;
          return AppWindowMaterial(state: material, child: content);
        },
        locale: language?.locale,
        localizationsDelegates: AppLocalization.delegates,
        supportedLocales: AppLocalization.supportedLocales,
        localeListResolutionCallback: AppLocalization.resolve,
        home: Builder(
          builder: (context) {
            final locale = Localizations.localeOf(context);
            AppStrings.useLocale(locale);
            modulesController?.useLocale(locale.toLanguageTag());
            final theme = Theme.of(context);
            final overlay = theme.brightness == Brightness.dark
                ? SystemUiOverlayStyle.light
                : SystemUiOverlayStyle.dark;
            Widget buildContent(bool windowReady) {
              final unifiedTitleBar = usesUnifiedMacosTitleBar(
                desktopWindow,
                windowReady: windowReady,
              );
              final shell = AppShell(
                controller: navigation,
                unifiedTitleBar: unifiedTitleBar,
                onCloseWindow: desktopWindow?.handleCloseRequested,
                onQuit: desktopWindow?.quit,
                onToggleFullScreen: desktopWindow?.toggleFullScreen,
                onExitFullScreen: desktopWindow?.exitFullScreen,
                onRefresh: historyController == null
                    ? null
                    : () {
                        if (navigation.selectedDestination ==
                            AppDestination.history) {
                          unawaited(historyController!.window.reload());
                        }
                      },
                headerActions: {
                  if (devicesController != null)
                    AppDestination.devices: [
                      DevicesHeaderActions(controller: devicesController!),
                    ],
                },
                headerStatus: [
                  if (settingsController != null) ...[
                    CaptureHeaderAction(controller: settingsController!),
                    SyncHeaderAction(
                      controller: settingsController!,
                      devices: devicesController,
                      onDrawerVisibilityChanged:
                          navigation.handleDrawerVisibilityChanged,
                    ),
                    StorageHeaderAction(
                      usage: settingsController!.storageUsage,
                      onPressed: () {
                        // The storage quota and its usage are in Clipboard.
                        settingsController!.showSection(
                          SettingsSectionId.clipboard,
                        );
                        navigation.selectDestination(AppDestination.settings);
                      },
                    ),
                  ],
                ],
                headerEnd: [
                  if (features case final features?)
                    PasteQueueHeaderAction(
                      key: features.pasteQueueAction,
                      controller: features.pasteQueue,
                      shortcuts: features.pasteQueueShortcuts,
                      onOpenSettings: () => features.openSettings(
                        features.pasteQueueSettingsSection,
                      ),
                      onDrawerVisibilityChanged:
                          navigation.handleDrawerVisibilityChanged,
                    ),
                  if (historyController?.transfers.fileOperations
                      case final transfers?)
                    FileOperationsHeaderAction(
                      controller: transfers,
                      onDrawerVisibilityChanged:
                          navigation.handleDrawerVisibilityChanged,
                      resolveDeviceClass: _deviceClassResolver(
                        devicesController,
                      ),
                    ),
                  if (notifications case final inbox?)
                    NotificationsHeaderAction(
                      controller: inbox,
                      onDrawerVisibilityChanged:
                          navigation.handleDrawerVisibilityChanged,
                    ),
                ],
                destinations: {
                  AppDestination.history: historyController == null
                      ? _withoutRuntime(
                          context,
                          context.l10n.appHistoryUnavailableTitle,
                        )
                      : HistoryScreen(
                          controller: historyController!,
                          findRequests: navigation.historyFindRequests,
                          onDrawerVisibilityChanged:
                              navigation.handleDrawerVisibilityChanged,
                        ),
                  AppDestination.devices: devicesController == null
                      ? _withoutRuntime(
                          context,
                          context.l10n.appDevicesUnavailableTitle,
                        )
                      : DevicesScreen(
                          controller: devicesController!,
                          onDrawerVisibilityChanged:
                              navigation.handleDrawerVisibilityChanged,
                          isActive: () =>
                              navigation.selectedDestination ==
                              AppDestination.devices,
                        ),
                  AppDestination.settings: settingsController == null
                      ? _withoutRuntime(
                          context,
                          context.l10n.appSettingsUnavailableTitle,
                        )
                      : SettingsScreen(
                          controller: settingsController!,
                          language: language,
                          quickPaste: quickPasteSettings,
                          launchAtLogin: launchAtLogin,
                          desktopVisibility: desktopWindow?.visibility,
                          appUpdate: appUpdateController,
                          modules: modulesController,
                          onOpenAndroidCaptureSetup: onOpenAndroidCaptureSetup,
                          onShowWelcomeTour: onShowWelcomeTour,
                          featureSections:
                              features?.settingsSections ?? const [],
                          moduleServices: features?.moduleServices,
                          storage: storage,
                          devicePreferences: devicePreferences,
                          appearance: appearance,
                          windowMaterial: windowMaterial,
                        ),
                },
              );

              if (desktopWindow == null) return shell;
              return ValueListenableBuilder<DesktopWindowSetupIssue?>(
                valueListenable: desktopWindow!.setupIssue,
                builder: (context, issue, child) {
                  // Without a tray the window still works; only window and
                  // quit failures replace the app.
                  if (issue == null ||
                      issue == DesktopWindowSetupIssue.trayUnavailable) {
                    return child!;
                  }
                  return SafeArea(
                    top: !unifiedTitleBar,
                    child: Scaffold(
                      resizeToAvoidBottomInset: true,
                      headers: unifiedTitleBar
                          ? const [MacosWindowHeader(title: Text('CopyPaste'))]
                          : const [AppBar(title: Text('CopyPaste')), Divider()],
                      footers: [
                        const Divider(),
                        Padding(
                          padding: const EdgeInsets.all(AppSpacing.lg),
                          child: Align(
                            alignment: Alignment.centerRight,
                            child: Button.secondary(
                              onPressed: desktopWindow!.quit,
                              child: Text(context.l10n.appQuit),
                            ),
                          ),
                        ),
                      ],
                      child: StateView.error(
                        title: context.l10n.appWindowSetupTitle,
                        message: issue.message,
                        onRetry: desktopWindow!.retry,
                      ),
                    ),
                  );
                },
                child: FailureToasts(
                  listenable: desktopWindow!.setupIssue,
                  failure: () {
                    final issue = desktopWindow!.setupIssue.value;
                    return issue == DesktopWindowSetupIssue.trayUnavailable
                        ? issue!.message
                        : null;
                  },
                  title: (context) => context.l10n.appWindowSetupTitle,
                  showInitial: true,
                  actionLabel: (context) => context.l10n.commonRetry,
                  onAction: () => unawaited(desktopWindow!.retry()),
                  child: shell,
                ),
              );
            }

            return ModulePromptPresenter(
              controller: modulePrompts,
              child: AnnotatedRegion<SystemUiOverlayStyle>(
                value: overlay.copyWith(
                  statusBarColor: Colors.transparent,
                  systemNavigationBarColor: theme.colorScheme.background,
                ),
                child: ColoredBox(
                  color: AppTheme.windowSurface(context),
                  child: desktopWindow == null
                      ? buildContent(false)
                      : ValueListenableBuilder<bool>(
                          valueListenable:
                              desktopWindow!.isUnifiedTitleBarReady,
                          builder: (context, windowReady, child) =>
                              buildContent(windowReady),
                        ),
                ),
              ),
            );
          },
        ),
      ),
    );
  }
}

DeviceClass? Function(String displayName)? _deviceClassResolver(
  DevicesController? controller,
) {
  if (controller == null) return null;
  return (displayName) {
    final snapshot = controller.snapshot;
    if (snapshot == null) return null;
    if (displayName == 'This device') {
      return snapshot.thisDevice.details?.profile?.deviceClass;
    }
    final candidates = <DeviceProfile?>[
      if (snapshot.thisDevice.name == displayName)
        snapshot.thisDevice.details?.profile,
      ...snapshot.peers
          .where((peer) => peer.name == displayName)
          .map((peer) => peer.details?.profile),
    ];
    if (candidates.length != 1) return null;
    return candidates.single?.deviceClass;
  };
}
