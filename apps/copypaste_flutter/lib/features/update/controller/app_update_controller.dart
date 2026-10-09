import 'dart:async';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:pub_semver/pub_semver.dart';

import '../../../platform/update/app_update_platform.dart';
import '../models/app_update_models.dart';
import '../repository/app_update_repository.dart';

class AppUpdateController extends ChangeNotifier {
  AppUpdateController({
    required AppUpdateRepository repository,
    required AppUpdatePlatform platform,
    Future<void> Function()? restart,
  }) : _repository = repository,
       _platform = platform,
       _restart = restart;

  final AppUpdateRepository _repository;
  final AppUpdatePlatform _platform;
  final Future<void> Function()? _restart;
  bool _restarting = false;

  AppUpdatePhase _phase = AppUpdatePhase.idle;
  Version? _currentVersion;
  AppRelease? _release;
  DownloadedAppUpdate? _downloaded;
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
        AppUpdatePhase.installing => true,
        _ => false,
      };

  Future<void> initialize() async {
    await check();
    final target = _platform.target;
    if (_disposed ||
        (target != AppUpdateTarget.android && target != AppUpdateTarget.linux)) {
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
      _fail('CopyPaste could not restore the update installation.');
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
          _currentVersion = installed;
          _release = null;
          _downloaded = null;
          _downloadProgress = 0;
          _message = availability.reason;
          _setPhase(AppUpdatePhase.unavailable);
          return;
        }
        final installation = availability.linuxInstallation;
        final repository = _repository;
        if (installation == null || repository is! LinuxAppUpdateRepository) {
          throw const AppUpdateException(
            'This Linux installation cannot update automatically.',
          );
        }
        release = await repository.findLinuxUpdate(
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
      _setPhase(AppUpdatePhase.available);
    } on FormatException {
      _fail('The installed CopyPaste version is invalid.');
    } on AppUpdateException catch (error) {
      _fail(error.message);
    } on SocketException {
      _fail('CopyPaste could not reach GitHub to check for updates.');
    } on TimeoutException {
      _fail('The update check timed out.');
    } on PlatformException {
      _fail('CopyPaste could not check for updates on this device.');
    } catch (_) {
      _fail('CopyPaste could not check for updates.');
    }
  }

  Future<void> install() async {
    final release = _release;
    if (release == null || busy || _disposed) return;
    try {
      var package = _downloaded;
      if (_platform.target != AppUpdateTarget.macos && package == null) {
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
      _fail('CopyPaste could not download the update.');
    } on TimeoutException {
      _fail('The update download timed out.');
    } on PlatformException catch (error) {
      _fail(_platformErrorMessage(error.code));
    } catch (_) {
      _fail('CopyPaste could not install the update.');
    }
  }

  Future<void> _applyInstallResult(AppUpdateInstallResult result) async {
    switch (result) {
      case AppUpdateInstallResult.started:
        _message = switch (_platform.target) {
          AppUpdateTarget.android => 'Continue in the Android system installer.',
          AppUpdateTarget.linux =>
            'Continue in your system package manager.',
          _ => 'The installer is starting.',
        };
        _setPhase(AppUpdatePhase.installing);
        break;
      case AppUpdateInstallResult.permissionRequired:
        _message =
            'Allow CopyPaste to install apps in Android settings, then continue.';
        _setPhase(AppUpdatePhase.permissionRequired);
        break;
      case AppUpdateInstallResult.restartRequired:
        _message = 'The update is installed. Restart CopyPaste to use it.';
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
    _message = 'Restarting CopyPaste.';
    notifyListeners();
    try {
      await restart();
    } catch (_) {
      _message = 'CopyPaste could not restart. Try again.';
    } finally {
      _restarting = false;
      if (!_disposed) notifyListeners();
    }
  }

  Future<void> openReleasePage() async {
    final release = _release;
    if (release == null) return;
    try {
      await _platform.openReleasePage(release.releaseUri);
    } on PlatformException {
      _fail('CopyPaste could not open the release page.');
    }
  }

  String _platformErrorMessage(String code) => switch (code) {
    'package_invalid' =>
      'The downloaded update is not a valid CopyPaste package.',
    'signature_invalid' => 'The downloaded update has an invalid signature.',
    'downgrade_refused' =>
      'The downloaded package is not newer than this version.',
    'homebrew_unavailable' =>
      'Install CopyPaste with Homebrew to update it here.',
    'installation_cancelled' => 'The update installation was cancelled.',
    'installation_blocked' => 'Android blocked the update installation.',
    'installation_conflict' => 'The update conflicts with the installed app.',
    'installation_incompatible' =>
      'The update is incompatible with this device.',
    'installation_storage' => 'Free up storage to install the update.',
    'installation_interrupted' =>
      'The update installation was interrupted. Try again.',
    'installation_busy' => 'An update installation is already in progress.',
    'invalid_arguments' => 'The update package details are invalid.',
    'unsupported_installation' =>
      'This Linux installation cannot update automatically.',
    'verification_failed' => 'The downloaded update failed its integrity check.',
    'installer_launch_failed' => 'CopyPaste could not start the system installer.',
    'update_busy' => 'An update installation is already in progress.',
    'open_failed' => 'CopyPaste could not open the release page.',
    _ => 'CopyPaste could not install the update.',
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
