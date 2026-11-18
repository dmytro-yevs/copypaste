import 'dart:io';

import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:file_selector/file_selector.dart';

import '../../../platform/files/history_file_picker.dart';
import '../../../platform/files/platform_file_gateway.dart';
import 'settings_repository.dart';

class FileSelectorSettingsFilePicker implements SettingsFilePicker {
  const FileSelectorSettingsFilePicker({
    this.fileGateway = const PlatformFileGateway(),
  });

  final PlatformFileGateway fileGateway;

  static XTypeGroup get _json => XTypeGroup(
    label: AppStrings.current.settingsRuntimeTextHistoryFileType,
    extensions: const <String>['json'],
  );
  static XTypeGroup get _backup => XTypeGroup(
    label: AppStrings.current.settingsRuntimeEncryptedBackupFileType,
    extensions: const <String>['copypaste-backup'],
  );
  @override
  Future<String?> chooseTextExportPath() async {
    if (Platform.isAndroid) {
      return fileGateway.androidStagingPath('history.json');
    }
    return (await getSaveLocation(
      suggestedName: 'copypaste-history.json',
      acceptedTypeGroups: [_json],
    ))?.path;
  }

  @override
  Future<String?> chooseBackupPath() async {
    if (Platform.isAndroid) {
      return fileGateway.androidStagingPath('history.copypaste-backup');
    }
    return (await getSaveLocation(
      suggestedName: 'copypaste-history.copypaste-backup',
      acceptedTypeGroups: [_backup],
    ))?.path;
  }

  @override
  Future<String?> chooseRestorePath() async {
    if (Platform.isAndroid) {
      final copy = await SystemHistoryFilePicker.copyChosenAndroidDocument(
        destination: (_) =>
            fileGateway.androidStagingPath('restore.copypaste-backup'),
      );
      return copy?.path;
    }
    return (await openFile(acceptedTypeGroups: [_backup]))?.path;
  }

  @override
  Future<String?> chooseFolder() => Platform.isAndroid
      ? fileGateway.chooseAndroidFolder()
      : getDirectoryPath();

  @override
  Future<void> presentCreatedFile(
    String path, {
    required String mimeType,
  }) async {
    await fileGateway.presentCreatedFile(path, mimeType: mimeType);
  }
}
