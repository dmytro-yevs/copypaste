import '../models/history_models.dart';

abstract interface class HistoryFileDownloader {
  Future<String?> chooseDestination(HistoryFileDetails file);

  Future<void> presentSavedFile(String path, HistoryFileDetails file);
}
