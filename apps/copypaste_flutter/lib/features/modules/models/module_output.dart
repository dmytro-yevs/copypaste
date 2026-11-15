import 'module_view.dart';

/// One entry of a Quick Paste surface.
class ModuleResultItem {
  const ModuleResultItem({
    required this.id,
    required this.title,
    required this.preview,
    this.badges = const [],
    this.secret = false,
    this.preselect = false,
    this.actions = const [],
  });

  /// Passed to the surface's activate command as `id`.
  final String id;
  final String title;
  final String preview;
  final List<String> badges;

  /// The preview is masked.
  final bool secret;
  final bool preselect;
  final List<ModuleViewAction> actions;
}

/// Text for the host to paste, insert, copy or queue.
class ModuleDelivery {
  const ModuleDelivery({
    required this.text,
    required this.label,
    this.secret = false,
    this.requireAuth = false,
    this.recordHistory = false,
    this.cursorFromEnd,
  });
  final String text;

  /// The provenance label of a recorded item.
  final String label;

  /// Written as a secret and never recorded.
  final bool secret;
  final bool requireAuth;
  final bool recordHistory;

  /// Where the caret goes after a paste, in graphemes from the end.
  final int? cursorFromEnd;
}

/// A file the module wrote under its data directory, for the host to save
/// where the user chooses as [name].
class ModuleSavedFile {
  const ModuleSavedFile({required this.path, required this.name});

  /// Relative to the module's data directory.
  final String path;
  final String name;
}
