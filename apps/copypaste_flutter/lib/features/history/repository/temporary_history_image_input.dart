import 'dart:io';

import 'package:path_provider/path_provider.dart';

import '../../modules/repository/modules_repository.dart';
import '../models/history_models.dart';
import 'history_image_input.dart';
import 'history_repository.dart';

class TemporaryHistoryImageInput implements HistoryImageInput {
  TemporaryHistoryImageInput({
    required HistoryRepository repository,
    Future<Directory> Function()? temporaryDirectory,
  }) : _repository = repository,
       _temporaryDirectory = temporaryDirectory ?? getTemporaryDirectory;

  final HistoryRepository _repository;
  final Future<Directory> Function() _temporaryDirectory;

  @override
  Future<SelectedModuleInput> prepare(HistoryClip clip) async {
    final root = await _temporaryDirectory();
    final directory = await root.createTemp('copypaste-ocr-');
    final extension = switch (clip.contentType) {
      'image/jpeg' => 'jpg',
      'image/webp' => 'webp',
      'image/tiff' => 'tiff',
      'image/bmp' => 'bmp',
      _ => 'png',
    };
    final name = 'image.$extension';
    final path = '${directory.path}${Platform.pathSeparator}$name';
    try {
      await _repository.saveFile(clip.id, path);
      return SelectedModuleInput(
        path: path,
        name: name,
        dispose: () => directory.delete(recursive: true),
      );
    } catch (_) {
      await directory.delete(recursive: true);
      rethrow;
    }
  }
}
