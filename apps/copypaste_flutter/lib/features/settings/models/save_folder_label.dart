/// A readable name for a save folder: the path, or for an Android tree URI
/// the folder inside its storage volume.
String saveFolderLabel(String folder) {
  if (!folder.startsWith('content://')) return folder;
  final uri = Uri.tryParse(folder);
  final segments = uri?.pathSegments ?? const <String>[];
  final tree = segments.indexOf('tree');
  if (tree < 0 || tree + 1 >= segments.length) return folder;
  final document = Uri.decodeComponent(segments[tree + 1]);
  final separator = document.indexOf(':');
  final path = separator < 0 ? document : document.substring(separator + 1);
  return path.isEmpty ? document : path;
}
