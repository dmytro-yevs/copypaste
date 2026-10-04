import 'dart:io';

import 'package:flutter/services.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../features/devices/devices.dart';
import '../features/history/controller/history_controller.dart';
import '../features/history/view/history_screen.dart';
import '../features/settings/controller/quick_paste_settings_controller.dart';
import '../features/settings/controller/settings_controller.dart';
import '../features/settings/view/capture_header_action.dart';
import '../features/settings/view/settings_screen.dart';
import '../platform/desktop/desktop_window_controller.dart';
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
    this.quickPasteSettings,
    this.settingsController,
    this.desktopWindow,
    this.onOpenAndroidCaptureSetup,
  });

  final AppNavigationController navigation;
  final HistoryController? historyController;
  final DevicesController? devicesController;
  final QuickPasteSettingsController? quickPasteSettings;
  final SettingsController? settingsController;
  final DesktopWindowController? desktopWindow;
  final Future<void> Function()? onOpenAndroidCaptureSetup;

  @override
  Widget build(BuildContext context) {
    return ShadcnApp(
      title: 'CopyPaste',
      theme: AppTheme.light,
      darkTheme: AppTheme.dark,
      themeMode: AppTheme.mode,
      builder: AppTheme.builder,
      localizationsDelegates: GlobalMaterialLocalizations.delegates,
      supportedLocales: ShadcnLocalizations.supportedLocales,
      home: Builder(
        builder: (context) {
          final theme = Theme.of(context);
          final overlay = theme.brightness == Brightness.dark
              ? SystemUiOverlayStyle.light
              : SystemUiOverlayStyle.dark;
          Widget buildContent(bool windowReady) {
            final unifiedTitleBar =
                Platform.isMacOS && desktopWindow != null && windowReady;
            final shell = AppShell(
              controller: navigation,
              unifiedTitleBar: unifiedTitleBar,
              headerActions: {
                if (settingsController != null)
                  AppDestination.history: [
                    CaptureHeaderAction(controller: settingsController!),
                  ],
                if (devicesController != null || settingsController != null)
                  AppDestination.devices: [
                    if (devicesController != null)
                      DevicesHeaderActions(controller: devicesController!),
                    if (settingsController != null)
                      CaptureHeaderAction(controller: settingsController!),
                  ],
                if (settingsController != null)
                  AppDestination.settings: [
                    CaptureHeaderAction(controller: settingsController!),
                  ],
              },
              destinations: {
                AppDestination.history: historyController == null
                    ? const StateView.error(
                        title: 'History runtime is unavailable',
                        message:
                            'Start the application runtime before opening history.',
                      )
                    : HistoryScreen(
                        controller: historyController!,
                        onDrawerVisibilityChanged:
                            navigation.setBottomOverlayOpen,
                      ),
                AppDestination.devices: devicesController == null
                    ? const StateView.error(
                        title: 'Devices runtime is unavailable',
                        message:
                            'Start the application runtime before opening devices.',
                      )
                    : DevicesScreen(
                        controller: devicesController!,
                        onDrawerVisibilityChanged:
                            navigation.setBottomOverlayOpen,
                        isActive: () =>
                            navigation.selectedDestination ==
                            AppDestination.devices,
                      ),
                AppDestination.settings: settingsController == null
                    ? const StateView.error(
                        title: 'Settings runtime is unavailable',
                        message:
                            'Start the application runtime before opening settings.',
                      )
                    : SettingsScreen(
                        controller: settingsController!,
                        quickPaste: quickPasteSettings,
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
                            child: const Text('Quit CopyPaste'),
                          ),
                        ),
                      ),
                    ],
                    child: StateView.error(
                      title: 'Window setup needs attention',
                      message: issue.message,
                      actionLabel: 'Try again',
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
