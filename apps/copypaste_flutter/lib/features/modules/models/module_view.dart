/// Host-rendered module output: titled sections of rows and text blocks.
class ModuleView {
  const ModuleView({required this.sections});
  final List<ModuleViewSection> sections;
}

class ModuleViewSection {
  const ModuleViewSection({this.title, required this.items});
  final String? title;
  final List<ModuleViewItem> items;
}

sealed class ModuleViewItem {
  const ModuleViewItem();
}

class ModuleViewRow extends ModuleViewItem {
  const ModuleViewRow({
    required this.title,
    this.subtitle,
    this.actions = const [],
  });
  final String title;
  final String? subtitle;
  final List<ModuleViewAction> actions;
}

/// Monospace text the user can copy.
class ModuleViewBlock extends ModuleViewItem {
  const ModuleViewBlock({this.label, required this.text});
  final String? label;
  final String text;
}

/// Invokes [command] of the same module with [arguments].
class ModuleViewAction {
  const ModuleViewAction({
    required this.title,
    required this.command,
    this.arguments = const {},
    this.destructive = false,
  });
  final String title;
  final String command;
  final Map<String, Object> arguments;
  final bool destructive;
}
