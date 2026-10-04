import 'dart:io';

import 'package:flutter/services.dart';

import '../../features/update/models/app_update_models.dart';

abstract interface class AppUpdatePlatform {
  AppUpdateTarget get target;

  Future<String> currentVersion();

  Future<AppUpdateAvailability> availability();

  Future<AppUpdateInstallResult> install({
    required AppRelease release,
    DownloadedAppUpdate? package,
  });

  Future<void> openReleasePage(Uri uri);
}

class MethodChannelAppUpdatePlatform implements AppUpdatePlatform {
  MethodChannelAppUpdatePlatform({MethodChannel? channel})
    : _channel = channel ?? const MethodChannel('com.copypaste.app/app_update');

  final MethodChannel _channel;

  @override
  AppUpdateTarget get target {
    if (Platform.isMacOS) return AppUpdateTarget.macos;
    if (Platform.isWindows) return AppUpdateTarget.windows;
    if (Platform.isAndroid) return AppUpdateTarget.android;
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
    return available
        ? const AppUpdateAvailability.available()
        : AppUpdateAvailability.unavailable(
            reason ??
                'Application updates are unavailable on this installation.',
          );
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
    return switch (result) {
      'started' => AppUpdateInstallResult.started,
      'permission_required' => AppUpdateInstallResult.permissionRequired,
      'restart_required' => AppUpdateInstallResult.restartRequired,
      _ => throw const AppUpdateException(
        'CopyPaste could not start the update.',
      ),
    };
  }

  @override
  Future<void> openReleasePage(Uri uri) =>
      _channel.invokeMethod<void>('openReleasePage', {'url': uri.toString()});
}
