import 'dart:io';

import 'package:file_selector/file_selector.dart';

import '../../../platform/files/platform_file_gateway.dart';
import 'settings_repository.dart';

class FileSelectorSettingsFilePicker implements SettingsFilePicker {
  const FileSelectorSettingsFilePicker({
    this.fileGateway = const PlatformFileGateway(),
  });

  final PlatformFileGateway fileGateway;

  static const _json = XTypeGroup(
    label: 'CopyPaste text history',
    extensions: <String>['json'],
  );
  static const _backup = XTypeGroup(
    label: 'CopyPaste encrypted backup',
    extensions: <String>['copypaste-backup'],
  );
  @override
  Future<String?> chooseTextExportPath() async {
    if (Platform.isAndroid) {
      return fileGateway.androidStagingPath('history.json');
    }
    return (await getSaveLocation(
      suggestedName: 'copypaste-history.json',
      acceptedTypeGroups: const [_json],
    ))?.path;
  }

  @override
  Future<String?> chooseBackupPath() async {
    if (Platform.isAndroid) {
      return fileGateway.androidStagingPath('history.copypaste-backup');
    }
    return (await getSaveLocation(
      suggestedName: 'copypaste-history.copypaste-backup',
      acceptedTypeGroups: const [_backup],
    ))?.path;
  }

  @override
  Future<String?> chooseRestorePath() async {
    final selected = await openFile(acceptedTypeGroups: const [_backup]);
    if (selected == null) return null;
    if (!Platform.isAndroid) return selected.path;
    final staged = await fileGateway.androidStagingPath(
      'restore.copypaste-backup',
    );
    await selected.saveTo(staged);
    return staged;
  }

  @override
  Future<void> presentCreatedFile(
    String path, {
    required String mimeType,
  }) async {
    await fileGateway.presentCreatedFile(path, mimeType: mimeType);
  }
}
