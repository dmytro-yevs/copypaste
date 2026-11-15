import 'dart:async';
import 'dart:io';
import 'dart:ui' show Rect;

import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:flutter/foundation.dart';
import 'package:mime/mime.dart';
import 'package:path_provider/path_provider.dart';

import '../../../platform/files/temporary_artifacts.dart';
import '../../../platform/share/platform_share_gateway.dart';
import '../../transfers/file_operations_controller.dart';
import '../models/history_models.dart';
import '../repository/history_file_downloader.dart';
import '../repository/history_file_importer.dart';
import '../repository/history_file_transfer.dart';
import '../repository/history_repository.dart';
import 'history_errors.dart';
import 'history_refresh_coordinator.dart';
import 'history_scope.dart';
import 'history_selection_controller.dart';

enum HistoryFileDownloadResult { saved, cancelled, failed }

/// How a share of several clips ended. [filesOnly] left their text out
/// because the share sheet does not take text together with files.
enum HistoryShareResult { shared, filesOnly, failed }

/// Clipboard copies, shares, file downloads and file imports with their
/// pending state.
class HistoryTransferController {
  HistoryTransferController(
    this._scope,
    this._errors,
    this._refresh, {
    required HistorySelectionController selection,
    required HistoryFileTransfer fileTransfer,
    HistoryFileDownloader? fileDownloader,
    HistoryFilePicker? filePicker,
    this.fileOperations,
    PlatformShareGateway share = const PlatformShareGateway(),
    Future<Directory> Function()? temporaryDirectory,
  }) : _selection = selection,
       _fileTransfer = fileTransfer,
       _fileDownloader = fileDownloader,
       _filePicker = filePicker,
       _share = share,
       _temporaryDirectory = temporaryDirectory ?? getTemporaryDirectory {
    fileOperations?.addListener(_fileOperationsChanged);
  }

  final HistoryScope _scope;
  final HistoryErrors _errors;
  final HistoryRefreshCoordinator _refresh;
  final HistorySelectionController _selection;
  final HistoryFileTransfer _fileTransfer;
  final HistoryFileDownloader? _fileDownloader;
  final HistoryFilePicker? _filePicker;
  final FileOperationsController? fileOperations;
  final PlatformShareGateway _share;
  final Future<Directory> Function() _temporaryDirectory;
  TemporaryDirectoryLease? _sharedImage;
  bool _isSharing = false;
  List<String> _shareFiles = const [];
  final Set<String> _sharePreparedFiles = <String>{};
  final Map<String, String> _downloadErrors = {};
  final Set<String> _downloadMutations = <String>{};
  final Set<String> _failedDownloads = <String>{};
  final Set<String> _cancelledDownloads = <String>{};
  final Map<String, double> _downloadProgress = <String, double>{};
  final ValueNotifier<int> _downloadProgressChanges = ValueNotifier<int>(0);
  String? _selectedTransferState;
  bool _isImportingFiles = false;

  bool get canImportFiles => _filePicker != null && !_scope.disposed;
  bool get isImportingFiles => _isImportingFiles;
  bool get hasPendingDownloads => _downloadMutations.isNotEmpty;
  bool get isSharing => _isSharing;

  /// How far a share of several clips has prepared its files, from 0 to 1;
  /// null while it waits for none. [downloadProgressChanges] tells changes.
  double? get shareFileProgress {
    final files = _shareFiles;
    if (!_isSharing || files.isEmpty) return null;
    var done = 0.0;
    for (final id in files) {
      done += _sharePreparedFiles.contains(id) ? 1 : _downloadProgress[id] ?? 0;
    }
    return done / files.length;
  }

  bool get canDownloadSelected {
    final file = _selection.selectedClip?.file;
    return _fileDownloader != null && file != null && !file.sourceAvailable;
  }

  bool isDownloadPending(String id) =>
      _downloadMutations.contains(id) ||
      (fileOperations?.forClip(id)?.active ?? false);
  bool downloadFailedFor(String id) =>
      fileOperations?.forClip(id)?.failed ?? _failedDownloads.contains(id);
  String? downloadErrorFor(String id) {
    final operation = fileOperations?.forClip(id);
    return operation == null ? _downloadErrors[id] : operation.error;
  }

  bool isDownloadCancelling(String id) => _cancelledDownloads.contains(id);
  double? downloadProgressFor(String id) =>
      fileOperations?.forClip(id)?.active == true
      ? fileOperations!.forClip(id)!.progress
      : _downloadProgress[id];
  Listenable get downloadProgressChanges => _downloadProgressChanges;

  void _fileOperationsChanged() {
    if (_scope.disposed) return;
    _downloadProgressChanges.value++;
    final operation = fileOperations?.forClip(_selection.selectedId ?? "");
    final state = "${operation?.id}/${operation?.phase}";
    if (state != _selectedTransferState) {
      _selectedTransferState = state;
      _scope.notify();
    }
  }

  String _fileFailure(Object error, String fallback) =>
      error is HistoryFileOperationException ? error.message : fallback;

  void cancelDownload(String id) {
    final operation = fileOperations?.forClip(id);
    if (operation?.active == true) {
      unawaited(fileOperations!.cancel(operation!));
      return;
    }
    if (!_downloadMutations.contains(id) || !_cancelledDownloads.add(id)) {
      return;
    }
    final repository = _scope.repository;
    if (repository is HistoryFileDownloadCancellation) {
      final cancellation = repository as HistoryFileDownloadCancellation;
      unawaited(
        Future<void>.sync(
          () => cancellation.cancelFileDownload(id),
        ).catchError((Object _) {}),
      );
    }
    _scope.notify();
  }

  Future<bool> copySelected({required bool plainText}) async {
    final clip = _selection.selectedClip;
    if (clip == null) {
      return false;
    }
    return copyClip(clip.id, plainText: plainText);
  }

  Future<bool> copyClip(String id, {bool plainText = false}) async {
    if (_scope.disposed) return false;
    try {
      if (plainText) {
        await _scope.repository.copyPlainText(id);
      } else {
        await _scope.repository.copy(id);
      }
      return true;
    } catch (error) {
      _errors.reportOperation(
        _fileFailure(
          error,
          plainText
              ? AppStrings.current.historyErrorCopyPlainText
              : AppStrings.current.historyErrorCopyClip,
        ),
        action: HistoryAction.copy,
      );
      _scope.notify();
      return false;
    }
  }

  Future<bool> copyTextFrom(String id, String text) async {
    if (_scope.disposed || text.isEmpty) return false;
    try {
      await _scope.repository.copyText(id, text);
      return true;
    } catch (_) {
      _errors.reportOperation(
        AppStrings.current.historyErrorCopyText,
        action: HistoryAction.copy,
      );
      if (!_scope.disposed) _scope.notify();
      return false;
    }
  }

  /// Whether the share sheet can take [clip]: text, an image or a file.
  bool canShare(HistoryClip clip) => switch (clip.contentKind) {
    HistoryClipKind.image => true,
    HistoryClipKind.file => clip.file != null,
    final kind => kind.isTextual,
  };

  /// Opens the native share sheet with [clip] as text, an image file or its
  /// file, downloading a remote file first. Returns false on failure.
  Future<bool> shareClip(HistoryClip clip, {Rect? origin}) async {
    // A repeated press while the sheet is being prepared is not a failure.
    if (_isSharing) return true;
    if (_scope.disposed || !canShare(clip)) return false;
    _isSharing = true;
    try {
      switch (clip.contentKind) {
        case HistoryClipKind.file:
          final path = await prepareFileForCopy(clip);
          if (path == null || _scope.disposed) return false;
          await _share.shareFile(
            path,
            mimeType: clip.file!.mimeType,
            origin: origin,
          );
        case HistoryClipKind.image:
          final path = (await _stageImages([clip])).single;
          await _share.shareFile(
            path,
            mimeType: clip.contentType,
            origin: origin,
          );
        default:
          final text =
              clip.body ??
              (await _scope.repository.get(clip.id)).body ??
              clip.preview;
          await _share.shareText(text, origin: origin);
      }
      return true;
    } catch (_) {
      return false;
    } finally {
      _isSharing = false;
    }
  }

  /// Opens one share sheet with the clips [ids] in their order. Text clips
  /// are joined with [separator]; images and files go as files, downloading
  /// files not on this device first. Text goes with files only where the
  /// sheet takes both, otherwise only the files are shared.
  Future<HistoryShareResult> shareClips(
    List<String> ids, {
    required String separator,
    Rect? origin,
  }) async {
    // A repeated press while the sheet is being prepared is not a failure.
    if (_isSharing) return HistoryShareResult.shared;
    if (_scope.disposed || ids.isEmpty) return HistoryShareResult.failed;
    _isSharing = true;
    _scope.notify();
    try {
      final clips = [
        for (final clip in await Future.wait(ids.map(_scope.repository.get)))
          if (canShare(clip)) clip,
      ];
      if (clips.isEmpty) return HistoryShareResult.failed;
      final texts = [
        for (final clip in clips)
          if (clip.contentKind.isTextual) clip.body ?? clip.preview,
      ];
      final attachments = [
        for (final clip in clips)
          if (!clip.contentKind.isTextual) clip,
      ];
      final paths = await _attachmentPaths(attachments);
      if (paths == null || _scope.disposed) return HistoryShareResult.failed;
      final text = texts.isEmpty ? null : texts.join(separator);
      if (paths.isEmpty) {
        await _share.shareText(text!, origin: origin);
        return HistoryShareResult.shared;
      }
      await _share.shareFiles(paths, text: text, origin: origin);
      return text == null || _share.sharesTextWithFiles
          ? HistoryShareResult.shared
          : HistoryShareResult.filesOnly;
    } catch (_) {
      return HistoryShareResult.failed;
    } finally {
      _isSharing = false;
      _shareFiles = const [];
      _sharePreparedFiles.clear();
      if (!_scope.disposed) _scope.notify();
    }
  }

  /// The paths of image and file [clips] in their order, or null when a file
  /// could not be prepared. Files download side by side.
  Future<List<String>?> _attachmentPaths(List<HistoryClip> clips) async {
    final images = await _stageImages([
      for (final clip in clips)
        if (clip.contentKind == HistoryClipKind.image) clip,
    ]);
    final files = [
      for (final clip in clips)
        if (clip.contentKind == HistoryClipKind.file) clip,
    ];
    _shareFiles = [for (final clip in files) clip.id];
    final filePaths = await Future.wait(
      files.map((clip) async {
        final path = await prepareFileForCopy(clip);
        _sharePreparedFiles.add(clip.id);
        if (!_scope.disposed) _downloadProgressChanges.value++;
        return path;
      }),
    );
    if (filePaths.contains(null)) return null;
    var image = 0;
    var file = 0;
    return [
      for (final clip in clips)
        clip.contentKind == HistoryClipKind.image
            ? images[image++]
            : filePaths[file++]!,
    ];
  }

  /// Writes [images] in order into one staging folder. The receiving app may
  /// read them after the sheet closes, so only the latest share stays staged
  /// until the next share or disposal.
  Future<List<String>> _stageImages(List<HistoryClip> images) async {
    await _releaseSharedImage();
    if (images.isEmpty) return const [];
    final lease = await TemporaryDirectoryLease.create(
      await _temporaryDirectory(),
      'copypaste-share-',
    );
    _sharedImage = lease;
    final paths = <String>[];
    for (final (index, clip) in images.indexed) {
      final extension = extensionFromMime(clip.contentType) ?? 'png';
      final name = images.length == 1
          ? 'image.$extension'
          : 'image-${index + 1}.$extension';
      final path = '${lease.directory.path}${Platform.pathSeparator}$name';
      await _scope.repository.saveFile(clip.id, path);
      paths.add(path);
    }
    if (_scope.disposed) {
      await _releaseSharedImage();
      throw StateError('History was closed.');
    }
    return paths;
  }

  Future<void> _releaseSharedImage() async {
    final lease = _sharedImage;
    _sharedImage = null;
    try {
      await lease?.dispose();
    } on FileSystemException {
      // The age-based staging cleanup removes it later.
    }
  }

  Future<String?> prepareFileForCopy(HistoryClip clip) async {
    if (!_downloadMutations.add(clip.id)) return null;
    _failedDownloads.remove(clip.id);
    _downloadErrors.remove(clip.id);
    _scope.notify();
    try {
      return await _fileTransfer.prepare(
        clip,
        onProgress: (progress) {
          if (_scope.disposed) return;
          _downloadProgress[clip.id] = progress;
          _downloadProgressChanges.value++;
        },
      );
    } catch (error) {
      if (!_scope.disposed) {
        _failedDownloads
          ..clear()
          ..add(clip.id);
        _downloadErrors
          ..clear()
          ..[clip.id] = _fileFailure(
            error,
            AppStrings.current.historyErrorPrepareFile,
          );
      }
      return null;
    } finally {
      _downloadMutations.remove(clip.id);
      _downloadProgress.remove(clip.id);
      if (!_scope.disposed) _scope.notify();
    }
  }

  Future<HistoryFileDownloadResult> downloadSelected() async {
    final clip = _selection.selectedClip;
    final file = clip?.file;
    final downloader = _fileDownloader;
    if (clip == null ||
        file == null ||
        downloader == null ||
        file.sourceAvailable ||
        !_downloadMutations.add(clip.id)) {
      return HistoryFileDownloadResult.failed;
    }
    _failedDownloads.remove(clip.id);
    _downloadErrors.remove(clip.id);
    _scope.notify();
    try {
      final destination = await downloader.chooseDestination(file);
      if (destination == null) return HistoryFileDownloadResult.cancelled;
      if (_cancelledDownloads.contains(clip.id)) {
        return HistoryFileDownloadResult.cancelled;
      }
      await _fileTransfer.saveTo(
        clip,
        destination,
        isCancelled: () => _cancelledDownloads.contains(clip.id),
        onProgress: (progress) {
          if (_scope.disposed) return;
          _downloadProgress[clip.id] = progress;
          _downloadProgressChanges.value++;
        },
      );
      if (_cancelledDownloads.contains(clip.id)) {
        await _fileTransfer.discard(destination);
        return HistoryFileDownloadResult.cancelled;
      }
      await downloader.presentSavedFile(destination, file);
      return HistoryFileDownloadResult.saved;
    } on HistoryFileTransferCancelled {
      return HistoryFileDownloadResult.cancelled;
    } catch (error) {
      if (_cancelledDownloads.contains(clip.id)) {
        return HistoryFileDownloadResult.cancelled;
      }
      _errors.reportOperation(
        _fileFailure(error, AppStrings.current.historyErrorDownloadFile),
        action: HistoryAction.download,
      );
      _failedDownloads.clear();
      _failedDownloads.add(clip.id);
      _downloadErrors
        ..clear()
        ..[clip.id] = _fileFailure(
          error,
          AppStrings.current.historyErrorDownloadFile,
        );
      _scope.notify();
      return HistoryFileDownloadResult.failed;
    } finally {
      _downloadMutations.remove(clip.id);
      _cancelledDownloads.remove(clip.id);
      _downloadProgress.remove(clip.id);
      if (!_scope.disposed) _scope.notify();
    }
  }

  Future<void> chooseFiles() async {
    final picker = _filePicker;
    if (picker == null || _scope.disposed || _isImportingFiles) return;
    _isImportingFiles = true;
    _scope.notify();
    String? failure;
    try {
      failure = await _importFiles(await picker.chooseFiles());
    } catch (error) {
      failure = _fileFailure(error, AppStrings.current.historyErrorSelectFiles);
    } finally {
      await _finishFileImport(failure);
    }
  }

  Future<void> importFiles(List<HistoryImportFile> files) async {
    if (_scope.disposed || _isImportingFiles) {
      for (final file in files) {
        await _releaseImportFile(file);
      }
      return;
    }
    _isImportingFiles = true;
    _scope.notify();
    String? failure;
    try {
      failure = await _importFiles(files);
    } finally {
      await _finishFileImport(failure);
    }
  }

  Future<String?> _importFiles(List<HistoryImportFile> files) async {
    if (files.isEmpty) return null;
    var failed = 0;
    String? failedName;
    String? firstReason;
    _errors.message = null;
    for (final file in files) {
      try {
        if (!_scope.disposed) {
          await _scope.repository.importFile(file);
          _refresh.defer();
        }
      } catch (error) {
        failed++;
        failedName ??= file.name;
        firstReason ??= _fileFailure(
          error,
          AppStrings.current.historyErrorReadSelectedFile,
        );
      } finally {
        await _releaseImportFile(file);
      }
    }
    return failed == 0
        ? null
        : AppStrings.current.historyErrorImportFiles(
            files.length,
            failed,
            failedName ?? '',
            firstReason ?? '',
          );
  }

  Future<void> _releaseImportFile(HistoryImportFile file) async {
    try {
      await file.dispose();
    } catch (_) {
      /* Best-effort temporary-file cleanup. */
    }
  }

  Future<void> _finishFileImport(String? failure) async {
    _isImportingFiles = false;
    if (_scope.disposed) return;
    await _refresh.flush();
    if (_scope.disposed) return;
    if (failure != null) {
      _errors.reportOperation(failure, action: HistoryAction.importFiles);
    }
    _scope.notify();
  }

  void dispose() {
    unawaited(_releaseSharedImage());
    fileOperations?.removeListener(_fileOperationsChanged);
    fileOperations?.dispose();
    _downloadProgress.clear();
    _downloadProgressChanges.dispose();
  }
}
