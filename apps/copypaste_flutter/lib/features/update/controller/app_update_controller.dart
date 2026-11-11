import 'dart:async';
import 'dart:io';

import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:pub_semver/pub_semver.dart';

import '../../../platform/update/app_update_platform.dart';
import '../models/app_update_models.dart';
import '../repository/app_update_repository.dart';

class AppUpdateController extends ChangeNotifier {
  static final Uri _releasePageFallback = Uri.https(
    'github.com',
    '/dmytro-yevs/copypaste/releases/latest',
  );

  AppUpdateController({
    required AppUpdateRepository repository,
    required AppUpdatePlatform platform,
    Future<void> Function()? restart,
    Future<void> Function(Version)? prepareModules,
  }) : _repository = repository,
       _platform = platform,
       _restart = restart,
       _prepareModules = prepareModules;

  final AppUpdateRepository _repository;
  final AppUpdatePlatform _platform;
  final Future<void> Function()? _restart;
  final Future<void> Function(Version)? _prepareModules;
  Version? _modulesPreparedFor;
  bool _restarting = false;

  AppUpdatePhase _phase = AppUpdatePhase.idle;
  Version? _currentVersion;
  AppRelease? _release;
  DownloadedAppUpdate? _downloaded;
  MacosAppUpdateInstallation? _macosInstallation;
  String? _message;
  double _downloadProgress = 0;
  bool _disposed = false;

  AppUpdatePhase get phase => _phase;
  Version? get currentVersion => _currentVersion;
  AppRelease? get release => _release;
  String? get message => _message;
  double get downloadProgress => _downloadProgress;
  bool get canRestart => _restart != null;
  bool get busy =>
      _restarting ||
      switch (_phase) {
        AppUpdatePhase.checking ||
        AppUpdatePhase.downloading ||
        AppUpdatePhase.preparingModules ||
        AppUpdatePhase.installing => true,
        _ => false,
      };

  Future<void> initialize() async {
    await check();
    final target = _platform.target;
    if (_disposed ||
        (target != AppUpdateTarget.android &&
            target != AppUpdateTarget.linux)) {
      return;
    }
    final previousPhase = _phase;
    _setPhase(AppUpdatePhase.installing);
    try {
      final result = await _platform.restoreInstallation();
      if (_disposed) return;
      if (result == null) {
        _setPhase(previousPhase);
      } else {
        await _applyInstallResult(result);
      }
    } on PlatformException catch (error) {
      _fail(_platformErrorMessage(error.code));
    } catch (_) {
      _fail(AppStrings.current.updateRestoreInstallationFailed);
    }
  }

  Future<void> check() async {
    if (busy || _disposed) return;
    _setPhase(AppUpdatePhase.checking);
    _message = null;
    try {
      final installed = Version.parse(await _platform.currentVersion());
      final target = _platform.target;
      AppUpdateAvailability? availability;
      final AppRelease? release;
      if (target == AppUpdateTarget.linux) {
        availability = await _platform.availability();
        if (!availability.available) {
          // Automatic package installation is optional on Linux. Keep a
          // verified GitHub release page available for manual installation.
          final fallbackRelease = await _repository.findUpdate(
            currentVersion: installed,
            target: target,
          );
          if (_disposed) return;
          _currentVersion = installed;
          _release = fallbackRelease;
          _downloaded = null;
          _downloadProgress = 0;
          _message = availability.reason;
          _setPhase(AppUpdatePhase.unavailable);
          return;
        }
        final installation = availability.linuxInstallation;
        final repository = _repository;
        if (installation == null || repository is! LinuxAppUpdateRepository) {
          throw AppUpdateException(
            AppStrings.current.updateLinuxAutomaticUnsupported,
          );
        }
        release = await (repository as LinuxAppUpdateRepository)
            .findLinuxUpdate(
              currentVersion: installed,
              installation: installation,
            );
      } else {
        release = await _repository.findUpdate(
          currentVersion: installed,
          target: target,
        );
      }
      if (_disposed) return;
      _currentVersion = installed;
      _release = release;
      _downloaded = null;
      _downloadProgress = 0;
      _macosInstallation = null;
      if (release == null) {
        _setPhase(AppUpdatePhase.upToDate);
        return;
      }
      availability ??= await _platform.availability();
      if (_disposed) return;
      if (!availability.available) {
        _message = availability.reason;
        _setPhase(AppUpdatePhase.unavailable);
        return;
      }
      _macosInstallation = availability.macosInstallation;
      if (_macosInstallation == MacosAppUpdateInstallation.bundle &&
          release.asset == null) {
        _message = AppStrings.current.updateMacosPackageMissing;
        _setPhase(AppUpdatePhase.unavailable);
        return;
      }
      _setPhase(AppUpdatePhase.available);
    } on FormatException {
      _fail(AppStrings.current.updateInstalledVersionInvalid);
    } on AppUpdateException catch (error) {
      _fail(error.message);
    } on SocketException {
      _fail(AppStrings.current.updateCheckUnreachable);
    } on TimeoutException {
      _fail(AppStrings.current.updateCheckTimedOut);
    } on PlatformException {
      _fail(AppStrings.current.updateCheckDeviceFailed);
    } catch (_) {
      _fail(AppStrings.current.updateCheckFailed);
    }
  }

  Future<void> install() async {
    final release = _release;
    if (release == null || busy || _disposed) return;
    try {
      var package = _downloaded;
      final downloads =
          _platform.target != AppUpdateTarget.macos ||
          _macosInstallation == MacosAppUpdateInstallation.bundle;
      if (downloads && package == null) {
        _downloadProgress = 0;
        _setPhase(AppUpdatePhase.downloading);
        package = await _repository.download(
          release,
          onProgress: (progress) {
            if (_disposed) return;
            _downloadProgress = progress.clamp(0, 1);
            notifyListeners();
          },
        );
        if (_disposed) return;
        _downloaded = package;
      }
      final prepareModules = _prepareModules;
      if (prepareModules != null && _modulesPreparedFor != release.version) {
        _setPhase(AppUpdatePhase.preparingModules);
        try {
          await prepareModules(release.version);
        } catch (_) {
          // Optional module updates must never prevent an application update.
        }
        if (_disposed) return;
        _modulesPreparedFor = release.version;
      }
      _setPhase(AppUpdatePhase.installing);
      _message = null;
      final result = await _platform.install(
        release: release,
        package: package,
      );
      if (_disposed) return;
      await _applyInstallResult(result);
    } on AppUpdateException catch (error) {
      _fail(error.message);
    } on SocketException {
      _fail(AppStrings.current.updateDownloadFailed);
    } on TimeoutException {
      _fail(AppStrings.current.updateDownloadTimedOut);
    } on PlatformException catch (error) {
      _fail(_platformErrorMessage(error.code));
    } catch (_) {
      _fail(AppStrings.current.updateInstallFailed);
    }
  }

  Future<void> _applyInstallResult(AppUpdateInstallResult result) async {
    switch (result) {
      case AppUpdateInstallResult.started:
        _message = switch (_platform.target) {
          AppUpdateTarget.android =>
            AppStrings.current.updateContinueInAndroidInstaller,
          AppUpdateTarget.linux =>
            AppStrings.current.updateContinueInPackageManager,
          _ => AppStrings.current.updateInstallerStarting,
        };
        _setPhase(AppUpdatePhase.installing);
        break;
      case AppUpdateInstallResult.permissionRequired:
        _message = switch (_platform.target) {
          AppUpdateTarget.android =>
            AppStrings.current.updatePermissionRequiredAndroid,
          AppUpdateTarget.linux =>
            AppStrings.current.updatePermissionRequiredLinux,
          _ => AppStrings.current.updatePermissionRequired,
        };
        _setPhase(AppUpdatePhase.permissionRequired);
        break;
      case AppUpdateInstallResult.restartRequired:
        _message = AppStrings.current.updateRestartRequired;
        _setPhase(AppUpdatePhase.restartRequired);
        break;
      case AppUpdateInstallResult.installed:
        _setPhase(AppUpdatePhase.idle);
        await check();
        break;
    }
  }

  Future<void> restartApplication() async {
    final restart = _restart;
    if (restart == null ||
        _phase != AppUpdatePhase.restartRequired ||
        busy ||
        _disposed) {
      return;
    }
    _restarting = true;
    _message = AppStrings.current.updateRestarting;
    notifyListeners();
    try {
      await restart();
    } catch (_) {
      _message = AppStrings.current.updateRestartFailed;
    } finally {
      _restarting = false;
      if (!_disposed) notifyListeners();
    }
  }

  Future<void> openReleasePage() async {
    try {
      await _platform.openReleasePage(
        _release?.releaseUri ?? _releasePageFallback,
      );
    } on PlatformException {
      _fail(AppStrings.current.updateReleasePageOpenFailed);
    }
  }

  String _platformErrorMessage(String code) => switch (code) {
    'package_invalid' => AppStrings.current.updatePackageInvalid,
    'signature_invalid' => AppStrings.current.updateSignatureInvalid,
    'downgrade_refused' => AppStrings.current.updateDowngradeRefused,
    'homebrew_unavailable' => AppStrings.current.updateHomebrewRequired,
    'installation_cancelled' => AppStrings.current.updateInstallationCancelled,
    'installation_blocked' => AppStrings.current.updateInstallationBlocked,
    'installation_conflict' => AppStrings.current.updateInstallationConflict,
    'installation_incompatible' =>
      AppStrings.current.updateInstallationIncompatible,
    'installation_storage' => AppStrings.current.updateInstallationStorage,
    'installation_interrupted' =>
      AppStrings.current.updateInstallationInterrupted,
    'installation_busy' => AppStrings.current.updateInstallationBusy,
    'invalid_arguments' => AppStrings.current.updatePackageDetailsInvalid,
    'unsupported_installation' =>
      AppStrings.current.updateLinuxAutomaticUnsupported,
    'verification_failed' => AppStrings.current.updateIntegrityCheckFailed,
    'installer_launch_failed' => AppStrings.current.updateInstallerLaunchFailed,
    'update_busy' => AppStrings.current.updateInstallationBusy,
    'install_location_translocated' =>
      AppStrings.current.updateInstallLocationTranslocated,
    'install_location_unwritable' =>
      AppStrings.current.updateInstallLocationUnwritable,
    'replacement_blocked' => AppStrings.current.updateReplacementBlocked,
    'open_failed' => AppStrings.current.updateReleasePageOpenFailed,
    _ => AppStrings.current.updateInstallFailed,
  };

  void _fail(String message) {
    if (_disposed) return;
    _message = message;
    _setPhase(AppUpdatePhase.error);
  }

  void _setPhase(AppUpdatePhase value) {
    if (_disposed) return;
    _phase = value;
    notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    _repository.dispose();
    super.dispose();
  }
}
