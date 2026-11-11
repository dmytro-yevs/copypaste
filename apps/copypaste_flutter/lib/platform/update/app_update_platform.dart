import 'dart:io';

import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:flutter/services.dart';

import '../../features/update/models/app_update_models.dart';
import 'app_update_architecture.dart';

abstract interface class AppUpdatePlatform {
  AppUpdateTarget get target;

  Future<String> currentVersion();

  Future<AppUpdateAvailability> availability();

  Future<AppUpdateInstallResult> install({
    required AppRelease release,
    DownloadedAppUpdate? package,
  });

  Future<AppUpdateInstallResult?> restoreInstallation();

  Future<void> openReleasePage(Uri uri);
}

class MethodChannelAppUpdatePlatform implements AppUpdatePlatform {
  MethodChannelAppUpdatePlatform({
    MethodChannel? channel,
    AppUpdateTarget? target,
    LinuxAppUpdateArchitecture? Function()? linuxArchitecture,
  }) : _channel =
           channel ?? const MethodChannel('com.copypaste.app/app_update'),
       _target = target,
       _linuxArchitecture = linuxArchitecture ?? currentLinuxUpdateArchitecture;

  final MethodChannel _channel;
  final AppUpdateTarget? _target;
  final LinuxAppUpdateArchitecture? Function() _linuxArchitecture;

  @override
  AppUpdateTarget get target {
    final configuredTarget = _target;
    if (configuredTarget != null) return configuredTarget;
    if (Platform.isMacOS) return AppUpdateTarget.macos;
    if (Platform.isWindows) return AppUpdateTarget.windows;
    if (Platform.isAndroid) return AppUpdateTarget.android;
    if (Platform.isLinux) return AppUpdateTarget.linux;
    throw AppUpdateException(AppStrings.current.platformUpdatesUnavailableHere);
  }

  @override
  Future<String> currentVersion() async {
    final version = await _channel.invokeMethod<String>('currentVersion');
    if (version == null || version.trim().isEmpty) {
      throw AppUpdateException(
        AppStrings.current.platformUpdateVersionUnavailable,
      );
    }
    return version.trim();
  }

  @override
  Future<AppUpdateAvailability> availability() async {
    final raw = await _channel.invokeMapMethod<String, Object?>('availability');
    if (raw == null) {
      return AppUpdateAvailability.unavailable(
        AppStrings.current.platformUpdatesUnavailableInstallation,
      );
    }
    final available = raw['available'] == true;
    final reason = raw['reason'] as String?;
    if (!available) {
      return AppUpdateAvailability.unavailable(
        reason ?? AppStrings.current.platformUpdatesUnavailableInstallation,
      );
    }
    if (target == AppUpdateTarget.linux) {
      final installation = LinuxAppUpdateInstallation.parse(
        installationType: raw['installationType'] as String?,
        architecture: raw['architecture'] as String?,
      );
      if (installation == null) {
        return AppUpdateAvailability.unavailable(
          AppStrings.current.platformUpdateLinuxUnsupported,
        );
      }
      if (_linuxArchitecture() != installation.architecture) {
        return AppUpdateAvailability.unavailable(
          AppStrings.current.platformUpdateLinuxArchitectureMismatch,
        );
      }
      return AppUpdateAvailability.available(linuxInstallation: installation);
    }
    if (target == AppUpdateTarget.macos) {
      return switch (raw['installationType']) {
        'homebrew' => const AppUpdateAvailability.available(
          macosInstallation: MacosAppUpdateInstallation.homebrew,
        ),
        'bundle' => const AppUpdateAvailability.available(
          macosInstallation: MacosAppUpdateInstallation.bundle,
        ),
        _ => AppUpdateAvailability.unavailable(
          AppStrings.current.platformUpdateMacosUnsupported,
        ),
      };
    }
    return const AppUpdateAvailability.available();
  }

  @override
  Future<AppUpdateInstallResult> install({
    required AppRelease release,
    DownloadedAppUpdate? package,
  }) async {
    final result = await _channel.invokeMethod<String>('install', {
      'version': release.version.toString(),
      if (package != null) 'path': package.path,
      if (package != null) 'sha256': package.asset.sha256,
    });
    return _installResult(result);
  }

  @override
  Future<AppUpdateInstallResult?> restoreInstallation() async {
    if (target != AppUpdateTarget.android && target != AppUpdateTarget.linux) {
      return null;
    }
    final result = await _channel.invokeMethod<String>('restoreInstallation');
    return result == null ? null : _installResult(result);
  }

  AppUpdateInstallResult _installResult(String? result) => switch (result) {
    'started' => AppUpdateInstallResult.started,
    'permission_required' => AppUpdateInstallResult.permissionRequired,
    'restart_required' => AppUpdateInstallResult.restartRequired,
    'installed' => AppUpdateInstallResult.installed,
    _ => throw AppUpdateException(AppStrings.current.platformUpdateStartFailed),
  };

  @override
  Future<void> openReleasePage(Uri uri) =>
      _channel.invokeMethod<void>('openReleasePage', {'url': uri.toString()});
}
