import 'dart:io';

/// Keep asserting that all payloads are gone while allowing the empty shared
/// staging namespace to remain for the next operation.
Future<List<FileSystemEntity>> temporaryPayloads(Directory root) async {
  final result = <FileSystemEntity>[];
  await for (final entry in root.list(followLinks: false)) {
    if (entry is Directory &&
        entry.path.split(Platform.pathSeparator).last == 'copypaste-staging') {
      result.addAll(await entry.list(followLinks: false).toList());
    } else {
      result.add(entry);
    }
  }
  return result;
}
