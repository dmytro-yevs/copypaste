import 'dart:async';

import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:flutter/foundation.dart';

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

/// Clipboard copies, file downloads and file imports with their pending state.
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
  }) : _selection = selection,
       _fileTransfer = fileTransfer,
       _fileDownloader = fileDownloader,
       _filePicker = filePicker {
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
    fileOperations?.removeListener(_fileOperationsChanged);
    fileOperations?.dispose();
    _downloadProgress.clear();
    _downloadProgressChanges.dispose();
  }
}
