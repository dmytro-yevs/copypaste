import 'dart:convert';
import 'dart:io';
import 'dart:math';

import '../../generated/api.dart' as runtime;
import '../../platform/files/platform_file_gateway.dart';
import '../../shared/runtime_event_service.dart';
import 'file_operations_controller.dart';

class RuntimeFileOperationsRepository implements FileOperationsRepository {
  const RuntimeFileOperationsRepository();
  @override
  Stream<FileOperationsChange> watch() => RuntimeEventService.instance.events
      .where((event) => event.kind == 'ready' || event.fileOperation != null)
      .map((event) {
        final operation = event.fileOperation;
        return FileOperationsChange(
          id: operation?.id,
          written: operation?.written.toInt() ?? 0,
          total: operation?.total.toInt() ?? 0,
          finished: operation?.phase != 'running',
        );
      });
  @override
  Future<List<FileOperation>> load({
    bool clear = false,
    String? before,
    String? id,
  }) async =>
      (jsonDecode(
                await runtime.fileOperations(
                  clear: clear,
                  before: before,
                  id: id,
                ),
              )
              as List)
          .map((value) {
            final record = Map<String, dynamic>.from(value as Map);
            return FileOperation(
              id: record['id'] as String,
              itemId: record['item_id'] as String,
              name: record['name'] as String,
              kind: FileOperationKind.values.byName(record['kind'] as String),
              phase: record['phase'] as String,
              sourceDevice: record['source_device'] as String,
              destinationDevice: record['destination_device'] as String,
              written: record['written'] as int,
              total: record['total'] as int,
              startedAt: DateTime.fromMillisecondsSinceEpoch(
                record['started_at'] as int,
              ),
              finishedAt: record['finished_at'] == null
                  ? null
                  : DateTime.fromMillisecondsSinceEpoch(
                      record['finished_at'] as int,
                    ),
              error: record['error'] as String?,
              localPath: record['local_path'] as String?,
            );
          })
          .toList(growable: false);
  @override
  Future<void> retry(FileOperation operation) async {
    final path = operation.localPath;
    if (path == null || operation.kind != FileOperationKind.receive) return;
    final token = base64UrlEncode(
      List.generate(16, (_) => Random.secure().nextInt(256)),
    );
    await runtime.startFileDownload(
      id: operation.itemId,
      destPath: path,
      token: token,
    );
  }

  @override
  Future<void> cancel(String id) => runtime.cancelClipFileDownload(id: id);
  @override
  Future<void> openFile(String path) async {
    if (!await File(path).exists()) {
      throw const FileSystemException('File unavailable');
    }
    await const PlatformFileGateway().openFile(path);
  }
}
