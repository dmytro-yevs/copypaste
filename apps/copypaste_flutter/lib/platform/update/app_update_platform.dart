import 'dart:io';

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
    throw const AppUpdateException('Application updates are unavailable here.');
  }

  @override
  Future<String> currentVersion() async {
    final version = await _channel.invokeMethod<String>('currentVersion');
    if (version == null || version.trim().isEmpty) {
      throw const AppUpdateException('The installed version is unavailable.');
    }
    return version.trim();
  }

  @override
  Future<AppUpdateAvailability> availability() async {
    final raw = await _channel.invokeMapMethod<String, Object?>('availability');
    if (raw == null) {
      return const AppUpdateAvailability.unavailable(
        'Application updates are unavailable on this installation.',
      );
    }
    final available = raw['available'] == true;
    final reason = raw['reason'] as String?;
    if (!available) {
      return AppUpdateAvailability.unavailable(
        reason ?? 'Application updates are unavailable on this installation.',
      );
    }
    if (target == AppUpdateTarget.linux) {
      final installation = LinuxAppUpdateInstallation.parse(
        installationType: raw['installationType'] as String?,
        architecture: raw['architecture'] as String?,
      );
      if (installation == null) {
        return const AppUpdateAvailability.unavailable(
          'This Linux installation cannot update automatically.',
        );
      }
      if (_linuxArchitecture() != installation.architecture) {
        return const AppUpdateAvailability.unavailable(
          'This Linux update does not match the running architecture.',
        );
      }
      return AppUpdateAvailability.available(linuxInstallation: installation);
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
    _ => throw const AppUpdateException(
      'CopyPaste could not start the update.',
    ),
  };

  @override
  Future<void> openReleasePage(Uri uri) =>
      _channel.invokeMethod<void>('openReleasePage', {'url': uri.toString()});
}
