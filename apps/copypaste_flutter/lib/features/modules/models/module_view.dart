import '../../../shared/forms/dynamic_form_draft.dart';

/// Host-rendered module output: titled sections of rows, text blocks and
/// library controls.
class ModuleView {
  const ModuleView({required this.sections});
  final List<ModuleViewSection> sections;
}

class ModuleViewSection {
  const ModuleViewSection({
    this.title,
    required this.items,
    this.bulkActions = const [],
    this.reorder,
    this.more,
  });
  final String? title;
  final List<ModuleViewItem> items;

  /// Actions on the selected rows, run with their ids as `ids`.
  final List<ModuleViewAction> bulkActions;

  /// Runs after a drag with `id` and `before`, the next row's id or `''`.
  final ModuleViewAction? reorder;

  /// Loads more rows; its view replaces this one.
  final ModuleViewAction? more;
}

/// Names a module may use for host icons.
enum ModuleIcon {
  notebookText('notebook_text'),
  filePlus('file_plus'),
  bookmark('bookmark'),
  sparkles('sparkles'),
  puzzle('puzzle'),
  zap('zap');

  const ModuleIcon(this.wireName);
  final String wireName;

  static ModuleIcon? fromWire(Object? name) =>
      values.where((icon) => icon.wireName == name).firstOrNull;
}

sealed class ModuleViewItem {
  const ModuleViewItem();
}

class ModuleViewRow extends ModuleViewItem {
  const ModuleViewRow({
    required this.title,
    this.subtitle,
    this.actions = const [],
    this.id,
    this.badges = const [],
    this.secret = false,
    this.icon,
    this.open,
  });
  final String title;
  final String? subtitle;
  final List<ModuleViewAction> actions;

  /// Needed for selection and reordering.
  final String? id;
  final List<String> badges;

  /// The subtitle is masked.
  final bool secret;
  final ModuleIcon? icon;

  /// Runs when the row itself is activated.
  final ModuleViewAction? open;
}

/// Monospace text the user can copy.
class ModuleViewBlock extends ModuleViewItem {
  const ModuleViewBlock({this.label, required this.text});
  final String? label;
  final String text;
}

/// A search field whose edits run [action] with `query`.
class ModuleViewSearch extends ModuleViewItem {
  const ModuleViewSearch({
    required this.query,
    required this.placeholder,
    required this.action,
  });
  final String query;
  final String placeholder;
  final ModuleViewAction action;
}

/// One choice among [options]; picking one runs [action] with `value`.
class ModuleViewFilter extends ModuleViewItem {
  const ModuleViewFilter({
    required this.options,
    required this.selected,
    required this.action,
  });
  final List<ModuleViewFilterOption> options;
  final String selected;
  final ModuleViewAction action;
}

class ModuleViewFilterOption {
  const ModuleViewFilterOption({
    required this.id,
    required this.title,
    this.count,
    this.icon,
  });
  final String id;
  final String title;
  final int? count;
  final ModuleIcon? icon;
}

class ModuleViewToolbar extends ModuleViewItem {
  const ModuleViewToolbar({required this.actions});
  final List<ModuleViewAction> actions;
}

enum ModuleNoticeTone { info, warning, error }

class ModuleViewNotice extends ModuleViewItem {
  const ModuleViewNotice({required this.text, required this.tone});
  final String text;
  final ModuleNoticeTone tone;
}

/// Inputs submitted together: [submit] and [preview] run with the field
/// values as `values`.
class ModuleViewForm extends ModuleViewItem {
  const ModuleViewForm({
    required this.fields,
    this.values = const {},
    this.errors = const {},
    required this.submit,
    this.preview,
  });
  final List<DynamicFormField> fields;
  final Map<String, Object> values;

  /// Messages shown under fields, by field id.
  final Map<String, String> errors;
  final ModuleViewAction submit;
  final ModuleViewAction? preview;
}

/// One of the module's preferences, rendered and saved by the host.
class ModuleViewPreference extends ModuleViewItem {
  const ModuleViewPreference({required this.id});
  final String id;
}

/// Where a view an action produces opens.
enum ModuleViewTarget { replace, drawer }

/// Invokes [command] of the same module with [arguments].
class ModuleViewAction {
  const ModuleViewAction({
    required this.title,
    required this.command,
    this.arguments = const {},
    this.destructive = false,
    this.target = ModuleViewTarget.replace,
    this.requireAuth = false,
    this.icon,
  });
  final String title;
  final String command;
  final Map<String, Object> arguments;
  final bool destructive;
  final ModuleViewTarget target;

  /// The device owner is verified first.
  final bool requireAuth;
  final ModuleIcon? icon;

  /// This action with the host-provided [extra] arguments added.
  ModuleViewAction withArguments(Map<String, Object> extra) => ModuleViewAction(
    title: title,
    command: command,
    arguments: Map.unmodifiable({...arguments, ...extra}),
    destructive: destructive,
    target: target,
    requireAuth: requireAuth,
    icon: icon,
  );
}
