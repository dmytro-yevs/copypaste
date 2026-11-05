import 'dart:async';

import 'package:flutter/foundation.dart';

import '../../transfers/file_operations_controller.dart';
import '../models/history_models.dart';
import '../repository/history_file_downloader.dart';
import '../repository/history_file_importer.dart';
import '../repository/history_file_transfer.dart';
import '../repository/history_repository.dart';
import 'history_bulk_controller.dart';
import 'history_errors.dart';
import 'history_media_cache.dart';
import 'history_mutation_controller.dart';
import 'history_ocr_controller.dart';
import 'history_pinned_order_controller.dart';
import 'history_refresh_coordinator.dart';
import 'history_scope.dart';
import 'history_selection_controller.dart';
import 'history_spoiler_controller.dart';
import 'history_tags_controller.dart';
import 'history_transfer_controller.dart';
import 'history_window_controller.dart';

/// Composes the History owners, notifies their shared listeners, and owns the
/// repository watch and the suspend/resume lifecycle.
class HistoryController extends ChangeNotifier {
  HistoryController(
    this._repository, {
    int pageSize = 50,
    bool includeFacets = true,
    bool separatePinnedWindow = false,
    Duration? searchDebounce,
    HistoryFileDownloader? fileDownloader,
    this.ocr,
    HistoryFilePicker? filePicker,
    HistoryFileTransfer? fileTransfer,
    FileOperationsController? fileOperations,
  }) {
    window = HistoryWindowController(
      _scope,
      this,
      errors: errors,
      spoilers: spoilers,
      tags: tags,
      media: media,
      refresh: refresh,
      ensureWatch: _ensureWatch,
      pageSize: pageSize,
      includeFacets: includeFacets,
      separatePinnedWindow: separatePinnedWindow,
      searchDebounce: searchDebounce ?? const Duration(milliseconds: 250),
    );
    transfers = HistoryTransferController(
      _scope,
      errors,
      refresh,
      selection: selection,
      fileTransfer: fileTransfer ?? HistoryFileTransfer(_repository),
      fileDownloader: fileDownloader,
      filePicker: filePicker,
      fileOperations: fileOperations,
    );
    tags.addListener(notifyListeners);
    spoilers.addListener(notifyListeners);
    ocr?.addListener(notifyListeners);
  }

  final HistoryRepository _repository;
  late final _scope = HistoryScope(_repository, notifyListeners);
  final errors = HistoryErrors();
  late final HistoryWindowController window;
  late final HistorySelectionController selection = HistorySelectionController(
    _scope,
    this,
    errors: errors,
    spoilers: spoilers,
  );
  late final HistoryBulkController bulk = HistoryBulkController(
    _scope,
    this,
    errors: errors,
    media: media,
    refresh: refresh,
  );
  late final HistoryMutationController mutations = HistoryMutationController(
    _scope,
    this,
    errors: errors,
    spoilers: spoilers,
    media: media,
  );
  late final HistoryPinnedOrderController pinnedOrder =
      HistoryPinnedOrderController(
        _scope,
        this,
        errors: errors,
        refresh: refresh,
      );
  late final HistoryTransferController transfers;
  late final HistoryRefreshCoordinator refresh = HistoryRefreshCoordinator(
    _scope,
    refresh: () => window.refreshVisible(),
    invalidate: () => window.invalidate(),
    held: () =>
        bulk.isBulkMutating ||
        bulk.isBulkDragSelecting ||
        transfers.isImportingFiles ||
        pinnedOrder.isInteracting,
    interrupted: () => bulk.isBulkDragSelecting || pinnedOrder.isInteracting,
  );
  late final HistoryMediaCache media = HistoryMediaCache(
    _scope,
    queryEpoch: () => window.queryEpoch,
    retainedSourceIconIds: () => {
      for (final clip in window.rows) ?clip.sourceAppIconId,
      ?selection.selectedClip?.sourceAppIconId,
      for (final app in window.facets.sourceApps) ?app.iconId,
    },
  );

  late final tags = HistoryTagsController(
    _repository is HistoryTagsRepository
        ? _repository as HistoryTagsRepository
        : null,
  );

  final _organizationChanges = ValueNotifier<int>(0);
  HistoryTagsController createTagsController() => HistoryTagsController(
    _repository is HistoryTagsRepository
        ? _repository as HistoryTagsRepository
        : null,
    changes: _organizationChanges,
  );

  final HistoryOcrController? ocr;
  late final spoilers = HistorySpoilerController(
    _repository is HistorySpoilerRepository
        ? _repository as HistorySpoilerRepository
        : null,
  );

  StreamSubscription<HistoryRuntimeEvent>? _watchSubscription;
  Future<void>? _watchCancellation;
  int _watchGeneration = 0;

  bool get canSuspend =>
      !bulk.isBulkDragSelecting &&
      !bulk.isBulkMutating &&
      !transfers.isImportingFiles &&
      !pinnedOrder.isInteracting &&
      !mutations.isDeletingAll &&
      !mutations.hasPending &&
      !transfers.hasPendingDownloads &&
      !(ocr?.busy ?? false);

  /// Releases loaded payloads while retaining the query, section state, and
  /// selection identity.
  void suspend() {
    if (_scope.disposed || _scope.suspended) return;
    _scope.suspended = true;
    unawaited(spoilers.setRevealed(null, false));
    bulk.suspend();
    _scope.generation++;
    window.suspend();
    tags.clear();
    selection.suspend();
    refresh.reset();
    media.clear();
    _cancelWatch();
  }

  Future<void> initialize() async {
    unawaited(transfers.fileOperations?.initialize());
    if (_scope.disposed) return;
    _scope.suspended = false;
    spoilers.disconnected();
    await _ensureWatch();
    await window.reloadWithFacets();
    await spoilers.refresh();
    await selection.awaitCurrentDetail();
  }

  /// The repository watch reconnects itself (RuntimeEventService owns the
  /// policy). Errors only mark History stale until the next change event.
  Future<void> _ensureWatch() async {
    await _watchCancellation;
    if (_scope.disposed || _scope.suspended) return;
    if (_watchSubscription != null) return;
    final generation = ++_watchGeneration;
    try {
      _watchSubscription = _repository.watch().listen(
        _onWatchEvent,
        onError: (Object error, StackTrace stackTrace) =>
            _watchErrored(generation),
        onDone: () => _watchEnded(generation),
      );
    } catch (_) {
      _watchEnded(generation);
    }
  }

  void _onWatchEvent(HistoryRuntimeEvent event) {
    if (_scope.suspended || _scope.disposed) return;
    if (event == HistoryRuntimeEvent.unavailable) {
      _watchErrored(_watchGeneration);
      return;
    }
    _organizationChanges.value++;
    unawaited(spoilers.refresh());
    if (refresh.hold()) {
      if (!bulk.isBulkMutating && !bulk.isBulkDragSelecting) {
        selection.reloadDetail();
      }
      return;
    }
    unawaited(refresh.run());
  }

  void _watchEnded(int generation) {
    if (generation != _watchGeneration) return;
    _watchSubscription = null;
    _watchUnavailable();
  }

  void _watchErrored(int generation) {
    if (generation != _watchGeneration) return;
    _watchUnavailable();
  }

  void _watchUnavailable() {
    if (_scope.disposed || _scope.suspended) return;
    spoilers.disconnected();
    errors.message = HistoryErrors.watchUnavailable;
    notifyListeners();
  }

  void _cancelWatch() {
    _watchGeneration++;
    final subscription = _watchSubscription;
    _watchSubscription = null;
    if (subscription == null) return;
    late final Future<void> cancellation;
    cancellation = subscription.cancel().catchError((Object _) {}).whenComplete(
      () {
        if (identical(_watchCancellation, cancellation)) {
          _watchCancellation = null;
        }
      },
    );
    _watchCancellation = cancellation;
    unawaited(cancellation);
  }

  @override
  void dispose() {
    _scope.disposed = true;
    transfers.dispose();
    media.dispose();
    spoilers.removeListener(notifyListeners);
    tags.removeListener(notifyListeners);
    tags.dispose();
    _organizationChanges.dispose();
    spoilers.dispose();
    ocr?.removeListener(notifyListeners);
    ocr?.dispose();
    window.dispose();
    _cancelWatch();
    super.dispose();
  }
}
