/// One user-selected file; platform adapters own access and temporary staging.
class HistoryImportFile {
  const HistoryImportFile({
    required this.name,
    required this.mimeType,
    this.sourceReference,
    required this.prepare,
    required this.dispose,
  });
  final String name;
  final String mimeType;
  final String? sourceReference;
  final Future<String> Function(int maxBytes) prepare;
  final Future<void> Function() dispose;
}

abstract interface class HistoryFilePicker {
  Future<List<HistoryImportFile>> chooseFiles();
}
