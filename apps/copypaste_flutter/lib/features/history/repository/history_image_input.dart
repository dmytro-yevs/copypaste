import '../../modules/repository/modules_repository.dart';
import '../models/history_models.dart';

/// Exposes the original image for the duration of a module invocation.
abstract interface class HistoryImageInput {
  Future<SelectedModuleInput> prepare(HistoryClip clip);
}
