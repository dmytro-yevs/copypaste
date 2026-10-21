import 'dart:io';

import 'package:copypaste_flutter/features/history/models/history_models.dart';
import 'package:copypaste_flutter/features/history/repository/history_file_transfer.dart';
import 'package:copypaste_flutter/features/history/repository/history_repository.dart';
import 'package:copypaste_flutter/features/transfers/file_operations_controller.dart';
import 'package:copypaste_flutter/features/transfers/runtime_file_operations_repository.dart';
import 'package:copypaste_flutter/generated/frb_generated.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test(
    'request failure preserves the runtime destination for manager retry',
    () async {
      final root = await Directory.systemTemp.createTemp('manager-retry-');
      addTearDown(() => root.delete(recursive: true));
      final history = _FailedHistory();
      final transfer = HistoryFileTransfer(
        history,
        temporaryDirectory: () async => root,
      );
      final clip = HistoryClip(
        id: 'retry-file',
        contentType: 'file',
        preview: 'report.pdf',
        createdAt: DateTime(2026),
        pinned: false,
        file: const HistoryFileDetails(name: 'report.pdf', sizeBytes: 4),
      );
      await expectLater(
        transfer.prepare(clip, onProgress: (_) {}),
        throwsStateError,
      );
      final path = history.destination!;
      expect(
        await File(path).parent.exists(),
        isTrue,
        reason: 'A request failure must not remove a runtime-owned destination',
      );
      final api = _DownloadApi();
      RustLib.initMock(api: api);
      addTearDown(RustLib.dispose);
      await const RuntimeFileOperationsRepository().retry(
        FileOperation(
          id: 'failed-attempt',
          itemId: clip.id,
          name: 'report.pdf',
          kind: FileOperationKind.receive,
          phase: 'failed',
          sourceDevice: 'Android',
          destinationDevice: 'Mac',
          written: 0,
          total: 4,
          startedAt: DateTime(2026),
          localPath: path,
          error: 'Source unavailable',
        ),
      );
      expect(api.itemId, clip.id);
      expect(await File(path).readAsBytes(), [1, 2, 3, 4]);
    },
  );

  test(
    'a late UI cancellation cannot roll back a runtime-committed file',
    () async {
      final root = await Directory.systemTemp.createTemp(
        'late-download-cancel-',
      );
      addTearDown(() => root.delete(recursive: true));
      final path = '${root.path}/report.pdf';
      var cancelled = false;
      await HistoryFileTransfer(_CommittedHistory()).saveTo(
        HistoryClip(
          id: 'committed',
          contentType: 'file',
          preview: 'report.pdf',
          createdAt: DateTime(2026),
          pinned: false,
          file: const HistoryFileDetails(name: 'report.pdf', sizeBytes: 4),
        ),
        path,
        onProgress: (progress) {
          if (progress >= 0.99) cancelled = true;
        },
        isCancelled: () => cancelled,
      );
      expect(await File(path).readAsBytes(), [1, 2, 3, 4]);
    },
  );
}

class _FailedHistory
    implements
        HistoryRepository,
        HistoryDownloadedFileRepository,
        HistoryFileDownloadCancellation {
  String? destination;
  @override
  Future<String?> downloadedFile(String id) async => null;
  @override
  Future<void> saveFile(String id, String path) async {
    destination = path;
    throw StateError('Source unavailable');
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _DownloadApi implements RustLibApi {
  String? itemId;
  @override
  Future<void> crateApiStartFileDownload({
    required String id,
    required String destPath,
    required String token,
  }) async {
    itemId = id;
    await File(destPath).writeAsBytes([1, 2, 3, 4]);
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _CommittedHistory extends _FailedHistory
    implements HistoryFileProgressRepository {
  @override
  Future<bool> saveFileWithProgress(
    String id,
    String path,
    void Function(int written, int total) onProgress,
  ) async {
    await File(path).writeAsBytes([1, 2, 3, 4]);
    onProgress(4, 4);
    return true;
  }
}
