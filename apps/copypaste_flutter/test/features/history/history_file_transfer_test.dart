import 'dart:async';
import 'dart:io';

import 'package:copypaste_flutter/features/history/models/history_models.dart';
import 'package:copypaste_flutter/features/history/repository/history_file_transfer.dart';
import 'package:copypaste_flutter/features/history/repository/history_repository.dart';
import 'package:flutter_test/flutter_test.dart';

HistoryClip clip({String name = 'original.bin'}) => HistoryClip(
  id: 'same-id',
  contentType: 'file',
  preview: 'original.bin',
  createdAt: DateTime.utc(2026),
  pinned: false,
  file: HistoryFileDetails(name: name, sizeBytes: 4),
);

void main() {
  test(
    'new presentations reuse the durable native-recorded file without saving again',
    () async {
      final root = await Directory.systemTemp.createTemp('durable-file-link-');
      addTearDown(() => root.delete(recursive: true));
      final existing = File('${root.path}/already-downloaded.bin');
      await existing.writeAsBytes([1, 2, 3, 4]);
      final repository = _DownloadedRepository(existing.path);
      final first = HistoryFileTransfer(
        repository,
        temporaryDirectory: () async => root,
      );
      final second = HistoryFileTransfer(
        repository,
        temporaryDirectory: () async => root,
      );
      expect(await first.prepare(clip(), onProgress: (_) {}), existing.path);
      expect(await second.prepare(clip(), onProgress: (_) {}), existing.path);
      expect(repository.saves, 0);
      await existing.delete();
      await second.prepare(clip(), onProgress: (_) {});
      expect(
        repository.saves,
        1,
        reason: 'A deleted local copy must be downloaded again',
      );
    },
  );

  test(
    'uses writer progress and confirms completion from the save result',
    () async {
      final repository = _ProgressRepository();
      final values = <double>[];
      final operation = HistoryFileTransfer(
        repository,
      ).saveTo(clip(), '/not-read-by-progress', onProgress: values.add);
      await Future<void>.delayed(Duration.zero);
      repository.progress!(2, 4);
      repository.progress!(4, 4);
      expect(values, [0, 0.5, 0.99]);
      repository.finish.complete();
      await operation;
      expect(values, [0, 0.5, 0.99, 1]);
      expect(repository.legacySaves, 0);
    },
  );

  test('legacy capability falls back to the existing save path', () async {
    final root = await Directory.systemTemp.createTemp('file-progress-legacy-');
    addTearDown(() => root.delete(recursive: true));
    final repository = _LegacyFallbackRepository();
    final values = <double>[];
    await HistoryFileTransfer(
      repository,
    ).saveTo(clip(), '${root.path}/saved.bin', onProgress: values.add);
    expect(repository.saves, 1);
    expect(values, [0, 1]);
    expect(await File('${root.path}/saved.bin').readAsBytes(), [1, 2, 3, 4]);
  });

  test(
    'reports progress while the destination is created and rolled back',
    () async {
      final root = await Directory.systemTemp.createTemp(
        'file-transfer-progress-',
      );
      addTearDown(() => root.delete(recursive: true));
      final repository = _WaitingRepository();
      final transfer = HistoryFileTransfer(repository);
      final destination = File('${root.path}/payload');
      final progress = <double>[];
      final halfway = Completer<void>();
      final operation = transfer.saveTo(
        clip(),
        destination.path,
        onProgress: (value) {
          progress.add(value);
          if (value == 0.5 && !halfway.isCompleted) halfway.complete();
        },
      );

      // Allow a poll while the writer has not created the file yet.
      await Future<void>.delayed(const Duration(milliseconds: 120));
      expect(progress, [0]);
      await destination.writeAsBytes([1, 2]);
      await halfway.future.timeout(const Duration(seconds: 5));
      await destination.delete();
      // Allow another poll during a writer rollback before it recreates the file.
      await Future<void>.delayed(const Duration(milliseconds: 120));
      await destination.writeAsBytes([1, 2, 3, 4]);
      repository.finish.complete();
      await operation;
      expect(progress.first, 0);
      expect(progress, contains(0.5));
      expect(progress.last, 1);
    },
  );

  test(
    'valid long original filenames stay below filesystem component limits',
    () async {
      final root = await Directory.systemTemp.createTemp(
        'file-transfer-long-name-',
      );
      addTearDown(() => root.delete(recursive: true));
      final transfer = HistoryFileTransfer(
        _Repository(),
        temporaryDirectory: () async => root,
      );
      final path = await transfer.prepare(
        clip(name: '${'a' * 190}.bin'),
        onProgress: (_) {},
      );
      expect(File(path).uri.pathSegments.last.length, lessThan(255));
      expect(path.endsWith('.bin'), isTrue);
      expect(await File(path).readAsBytes(), [1, 2, 3, 4]);
    },
  );
  test(
    'same id and size with changed bytes keeps both original publications',
    () async {
      final root = await Directory.systemTemp.createTemp(
        'file-transfer-version-',
      );
      addTearDown(() => root.delete(recursive: true));
      final repository = _Repository();
      final transfer = HistoryFileTransfer(
        repository,
        temporaryDirectory: () async => root,
      );
      final first = clip();
      final original = await transfer.prepare(first, onProgress: (_) {});
      expect(await transfer.prepare(first, onProgress: (_) {}), original);
      expect(repository.saves, 1);
      repository.bytes = [5, 6, 7, 8];
      final updated = await transfer.prepare(clip(), onProgress: (_) {});
      expect(updated, isNot(original));
      expect(await File(original).readAsBytes(), [1, 2, 3, 4]);
      expect(await File(updated).readAsBytes(), [5, 6, 7, 8]);
    },
  );

  test(
    'cleanup repeats after the first action and failed saves preserve existing files',
    () async {
      final root = await Directory.systemTemp.createTemp(
        'file-transfer-cleanup-',
      );
      addTearDown(() => root.delete(recursive: true));
      final repository = _Repository();
      final transfer = HistoryFileTransfer(
        repository,
        temporaryDirectory: () async => root,
      );
      final first = clip();
      final original = await transfer.prepare(first, onProgress: (_) {});
      final abandoned = File('${File(original).parent.path}/abandoned.bin');
      await abandoned.writeAsBytes([9]);
      await abandoned.setLastModified(
        DateTime.now().subtract(const Duration(days: 3)),
      );
      await transfer.prepare(first, onProgress: (_) {});
      expect(await abandoned.exists(), isFalse);
      repository.fail = true;
      await expectLater(
        transfer.prepare(clip(), onProgress: (_) {}),
        throwsStateError,
      );
      expect(await File(original).readAsBytes(), [1, 2, 3, 4]);
      expect(
        await Directory('${root.path}/copypaste-staging')
            .list()
            .where(
              (entry) =>
                  entry is Directory &&
                  entry.path.contains('copypaste-transfer-'),
            )
            .isEmpty,
        isTrue,
      );
    },
  );
}

class _WaitingRepository implements HistoryRepository {
  final finish = Completer<void>();

  @override
  Future<void> saveFile(String id, String path) => finish.future;

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Repository implements HistoryRepository {
  List<int> bytes = [1, 2, 3, 4];
  int saves = 0;
  bool fail = false;

  @override
  Future<void> saveFile(String id, String path) async {
    saves++;
    await File(path).writeAsBytes(bytes);
    if (fail) throw StateError('injected failure');
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _ProgressRepository
    implements HistoryRepository, HistoryFileProgressRepository {
  final finish = Completer<void>();
  void Function(int, int)? progress;
  int legacySaves = 0;

  @override
  Future<bool> saveFileWithProgress(
    String id,
    String path,
    void Function(int, int) onProgress,
  ) async {
    progress = onProgress;
    await finish.future;
    return true;
  }

  @override
  Future<void> saveFile(String id, String path) async {
    legacySaves++;
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _LegacyFallbackRepository extends _Repository
    implements HistoryFileProgressRepository {
  @override
  Future<bool> saveFileWithProgress(
    String id,
    String path,
    void Function(int, int) progress,
  ) async => false;
}

class _DownloadedRepository extends _Repository
    implements HistoryDownloadedFileRepository {
  _DownloadedRepository(this.path);
  final String path;
  @override
  Future<String?> downloadedFile(String id) async =>
      await File(path).exists() ? path : null;
}
