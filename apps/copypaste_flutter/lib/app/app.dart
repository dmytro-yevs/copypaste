import 'dart:async';

import '../features/modules/controller/modules_controller.dart';

import '../features/transfers/file_operations_view.dart';
import '../features/notifications/notifications_controller.dart';
import '../features/notifications/notifications_view.dart';

import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../features/devices/devices.dart';
import '../l10n/l10n.dart';
import 'localization/app_language.dart';
import '../features/history/controller/history_controller.dart';
import '../features/history/view/history_screen.dart';
import '../features/settings/controller/quick_paste_settings_controller.dart';
import '../features/settings/controller/settings_controller.dart';
import '../features/settings/controller/launch_at_login_controller.dart';
import '../features/settings/view/capture_header_action.dart';
import '../features/settings/view/sync_header_action.dart';
import '../features/settings/view/storage_header_action.dart';
import '../features/settings/controller/settings_navigation_state.dart';
import '../features/settings/view/settings_screen.dart';
import '../features/update/controller/app_update_controller.dart';
import '../platform/desktop/desktop_window_controller.dart';
import '../platform/desktop/desktop_shortcuts.dart';
import '../shared/state_view.dart';
import 'navigation/navigation.dart';
import 'shell/shell.dart';
import 'theme/app_theme.dart';
import 'theme/app_tokens.dart';

class CopyPasteApp extends StatelessWidget {
  const CopyPasteApp({
    super.key,
    required this.navigation,
    this.historyController,
    this.devicesController,
    this.notifications,
    this.quickPasteSettings,
    this.settingsController,
    this.launchAtLogin,
    this.modulesController,
    this.appUpdateController,
    this.desktopWindow,
    this.runtimeUnavailableMessage,
    this.onRetryRuntime,
    this.onOpenAndroidCaptureSetup,
    this.navigatorKey,
    this.language,
  });

  final AppNavigationController navigation;
  final HistoryController? historyController;
  final DevicesController? devicesController;
  final NotificationsController? notifications;
  final QuickPasteSettingsController? quickPasteSettings;
  final SettingsController? settingsController;
  final LaunchAtLoginController? launchAtLogin;
  final ModulesController? modulesController;
  final AppUpdateController? appUpdateController;
  final DesktopWindowController? desktopWindow;
  final String? runtimeUnavailableMessage;
  final Future<void> Function()? onRetryRuntime;
  final Future<void> Function()? onOpenAndroidCaptureSetup;
  final GlobalKey<NavigatorState>? navigatorKey;
  final AppLanguageController? language;

  @override
  Widget build(BuildContext context) {
    return ShadcnApp(
      navigatorKey: navigatorKey,
      title: 'CopyPaste',
      // The debug ribbon covers the header's end controls.
      debugShowCheckedModeBanner: false,
      theme: AppTheme.light,
      darkTheme: AppTheme.dark,
      themeMode: AppTheme.mode,
      builder: (context, child) => AppTheme.builder(
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
      ),
      locale: language?.locale,
      localizationsDelegates: AppLocalization.delegates,
      supportedLocales: AppLocalization.supportedLocales,
      localeListResolutionCallback: AppLocalization.resolve,
      home: Builder(
        builder: (context) {
          AppStrings.useLocale(Localizations.localeOf(context));
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
                if (historyController?.transfers.fileOperations
                    case final transfers?)
                  FileOperationsHeaderAction(
                    controller: transfers,
                    onDrawerVisibilityChanged:
                        navigation.handleDrawerVisibilityChanged,
                    resolveDeviceClass: _deviceClassResolver(devicesController),
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
                    ? StateView.error(
                        title: context.l10n.appHistoryUnavailableTitle,
                        message:
                            runtimeUnavailableMessage ??
                            context.l10n.appHistoryUnavailableMessage,
                        actionLabel: onRetryRuntime == null
                            ? null
                            : context.l10n.commonRetry,
                        onAction: onRetryRuntime,
                      )
                    : HistoryScreen(
                        controller: historyController!,
                        findRequests: navigation.historyFindRequests,
                        onDrawerVisibilityChanged:
                            navigation.handleDrawerVisibilityChanged,
                      ),
                AppDestination.devices: devicesController == null
                    ? StateView.error(
                        title: context.l10n.appDevicesUnavailableTitle,
                        message:
                            runtimeUnavailableMessage ??
                            context.l10n.appDevicesUnavailableMessage,
                        actionLabel: onRetryRuntime == null
                            ? null
                            : context.l10n.commonRetry,
                        onAction: onRetryRuntime,
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
                    ? StateView.error(
                        title: context.l10n.appSettingsUnavailableTitle,
                        message:
                            runtimeUnavailableMessage ??
                            context.l10n.appSettingsUnavailableMessage,
                        actionLabel: onRetryRuntime == null
                            ? null
                            : context.l10n.commonRetry,
                        onAction: onRetryRuntime,
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
                      ),
              },
            );

            if (desktopWindow == null) return shell;
            return ValueListenableBuilder<DesktopWindowSetupIssue?>(
              valueListenable: desktopWindow!.setupIssue,
              builder: (context, issue, child) {
                if (issue == null) return child!;
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
                      actionLabel: context.l10n.commonTryAgain,
                      onAction: desktopWindow!.retry,
                    ),
                  ),
                );
              },
              child: shell,
            );
          }

          return AnnotatedRegion<SystemUiOverlayStyle>(
            value: overlay.copyWith(
              statusBarColor: Colors.transparent,
              systemNavigationBarColor: theme.colorScheme.background,
            ),
            child: ColoredBox(
              color: theme.colorScheme.background,
              child: desktopWindow == null
                  ? buildContent(false)
                  : ValueListenableBuilder<bool>(
                      valueListenable: desktopWindow!.isUnifiedTitleBarReady,
                      builder: (context, windowReady, child) =>
                          buildContent(windowReady),
                    ),
            ),
          );
        },
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
