import '../../../l10n/l10n.dart';
import '../models/module_models.dart';
import '../models/module_output.dart';
import '../models/module_surfaces.dart';
import '../models/module_view.dart';

/// The JSON the host serves for modules: fields, surfaces, views and
/// command outputs. Optional newer values that this app does not know are
/// skipped or fall back to their defaults.
abstract final class ModuleWire {
  /// The most UTF-8 bytes of a text value without its own limit.
  static const defaultTextBytes = 64 * 1024;

  /// [inForm] fields belong to a view form, where text is single-line
  /// unless it is `multiline`; elsewhere text keeps its multi-line input.
  static List<ModuleField> fields(Object? value, {bool inForm = false}) =>
      List.unmodifiable(
        (value as List<dynamic>).map(
          (value) => _field(value as Map<String, dynamic>, inForm: inForm),
        ),
      );

  static ModuleField _field(
    Map<String, dynamic> field, {
    required bool inForm,
  }) {
    final kind = switch (field['kind']) {
      'text' when inForm && field['multiline'] != true => ModuleFieldKind.line,
      'text' => ModuleFieldKind.text,
      'boolean' => ModuleFieldKind.boolean,
      'file' => ModuleFieldKind.file,
      'choices' => ModuleFieldKind.choices,
      'device_set' => ModuleFieldKind.devices,
      'source_app_set' => ModuleFieldKind.applications,
      'id_list' => ModuleFieldKind.ids,
      'values' => ModuleFieldKind.formValues,
      _ => throw ModulesException(AppStrings.current.modulesUnsupportedField),
    };
    return ModuleField(
      id: field['id'] as String,
      title: field['title'] as String,
      options: List.unmodifiable(
        (field['options'] as List<dynamic>? ?? const []).map((value) {
          final option = value as Map<String, dynamic>;
          return ModuleChoice(
            id: option['id'] as String,
            title: option['title'] as String,
          );
        }),
      ),
      kind: kind,
      secret: field['secret'] as bool? ?? false,
      template: field['template'] as bool? ?? false,
      defaultValue: switch (kind) {
        ModuleFieldKind.file => '',
        ModuleFieldKind.devices ||
        ModuleFieldKind.applications ||
        ModuleFieldKind.ids => const <String>[],
        ModuleFieldKind.formValues => const <String, Object>{},
        _ => field['default'] as Object,
      },
      acceptedExtensions: List.unmodifiable(
        (field['accepted_extensions'] as List<dynamic>? ?? const [])
            .cast<String>(),
      ),
      maxBytes:
          field['max_bytes'] as int? ??
          (kind == ModuleFieldKind.text || kind == ModuleFieldKind.line
              ? defaultTextBytes
              : 64 * 1024 * 1024),
      required: field['required'] as bool? ?? false,
    );
  }

  static ModuleSurfaces surfaces(Object? value) {
    if (value is! Map<String, dynamic>) return const ModuleSurfaces();
    final settings = value['settings'] as Map<String, dynamic>?;
    final quickPaste = value['quick_paste'] as Map<String, dynamic>?;
    return ModuleSurfaces(
      settings: settings == null
          ? null
          : ModuleSettingsSurface(
              command: settings['command'] as String,
              icon: ModuleIcon.fromWire(settings['icon']),
            ),
      quickPaste: quickPaste == null
          ? null
          : ModuleQuickPasteSurface(
              title: quickPaste['title'] as String,
              searchCommand: quickPaste['search_command'] as String,
              activateCommand: quickPaste['activate_command'] as String,
              mixedLimit: quickPaste['mixed_limit'] as int? ?? 0,
            ),
      itemActions: List.unmodifiable(
        _maps(value['item_actions']).map(
          (action) => ModuleItemAction(
            id: action['id'] as String,
            title: action['title'] as String,
            command: action['command'] as String,
            icon: ModuleIcon.fromWire(action['icon']),
          ),
        ),
      ),
      trayActions: List.unmodifiable(
        _maps(value['tray_actions']).map(
          (action) => ModuleTrayAction(
            id: action['id'] as String,
            title: action['title'] as String,
            command: action['command'] as String,
          ),
        ),
      ),
    );
  }

  /// A command output; unknown kinds are refused.
  static ModuleResult output(
    Map<String, dynamic> result,
  ) => switch (result['kind']) {
    'text' => ModuleResult(result['text'] as String),
    'view' => ModuleResult.view(view(result['view'] as Map<String, dynamic>)),
    'message' => ModuleResult(result['message'] as String, message: true),
    'data'
        when result['data'] is Map &&
            (result['data'] as Map)['message'] is String =>
      ModuleResult((result['data'] as Map)['message'] as String, message: true),
    'results' => ModuleResult.results(
      List.unmodifiable(_maps(result['items']).map(_resultItem)),
    ),
    'deliver' => ModuleResult.delivery(
      _delivery(result['delivery'] as Map<String, dynamic>),
    ),
    'save_file' => ModuleResult.file(
      ModuleSavedFile(
        path: result['path'] as String,
        name: result['name'] as String,
      ),
    ),
    _ => throw ModulesException(AppStrings.current.modulesUnsupportedResult),
  };

  static ModuleView view(Map<String, dynamic> view) => ModuleView(
    sections: List.unmodifiable(
      _maps(view['sections']).map(
        (section) => ModuleViewSection(
          title: section['title'] as String?,
          items: List.unmodifiable(_maps(section['items']).map(_item).nonNulls),
          bulkActions: _actions(section['bulk_actions']),
          reorder: _optionalAction(section['reorder']),
          more: _optionalAction(section['more']),
        ),
      ),
    ),
  );

  static ModuleViewItem? _item(Map<String, dynamic> item) =>
      switch (item['kind']) {
        'row' => ModuleViewRow(
          title: item['title'] as String,
          subtitle: item['subtitle'] as String?,
          actions: _actions(item['actions']),
          id: item['id'] as String?,
          badges: _strings(item['badges']),
          secret: item['secret'] as bool? ?? false,
          icon: ModuleIcon.fromWire(item['icon']),
          open: _optionalAction(item['open']),
        ),
        'block' => ModuleViewBlock(
          label: item['label'] as String?,
          text: item['text'] as String,
        ),
        'search' => ModuleViewSearch(
          query: item['query'] as String,
          placeholder: item['placeholder'] as String,
          action: _action(item['action'] as Map<String, dynamic>),
        ),
        'filter' => ModuleViewFilter(
          options: List.unmodifiable(
            _maps(item['options']).map(
              (option) => ModuleViewFilterOption(
                id: option['id'] as String,
                title: option['title'] as String,
                count: option['count'] as int?,
                icon: ModuleIcon.fromWire(option['icon']),
              ),
            ),
          ),
          selected: item['selected'] as String,
          action: _action(item['action'] as Map<String, dynamic>),
        ),
        'toolbar' => ModuleViewToolbar(actions: _actions(item['actions'])),
        'notice' => ModuleViewNotice(
          text: item['text'] as String,
          tone: switch (item['tone']) {
            'warning' => ModuleNoticeTone.warning,
            'error' => ModuleNoticeTone.error,
            _ => ModuleNoticeTone.info,
          },
        ),
        'form' => ModuleViewForm(
          fields: fields(item['fields'], inForm: true),
          values: Map.unmodifiable(
            (item['values'] as Map<String, dynamic>? ?? const {})
                .cast<String, Object>(),
          ),
          errors: Map.unmodifiable(
            (item['errors'] as Map<String, dynamic>? ?? const {})
                .cast<String, String>(),
          ),
          submit: _action(item['submit'] as Map<String, dynamic>),
          preview: _optionalAction(item['preview']),
        ),
        'preference' => ModuleViewPreference(id: item['id'] as String),
        _ => null,
      };

  static ModuleViewAction? _optionalAction(Object? value) =>
      value == null ? null : _action(value as Map<String, dynamic>);

  static List<ModuleViewAction> _actions(Object? value) =>
      List.unmodifiable(_maps(value).map(_action));

  static ModuleViewAction _action(Map<String, dynamic> action) =>
      ModuleViewAction(
        title: action['title'] as String,
        command: action['command'] as String,
        arguments: Map.unmodifiable(
          (action['arguments'] as Map<String, dynamic>? ?? const {})
              .cast<String, Object>(),
        ),
        destructive: action['destructive'] as bool? ?? false,
        target: action['target'] == 'drawer'
            ? ModuleViewTarget.drawer
            : ModuleViewTarget.replace,
        requireAuth: action['require_auth'] as bool? ?? false,
        icon: ModuleIcon.fromWire(action['icon']),
      );

  static ModuleResultItem _resultItem(Map<String, dynamic> item) =>
      ModuleResultItem(
        id: item['id'] as String,
        title: item['title'] as String,
        preview: item['preview'] as String,
        badges: _strings(item['badges']),
        secret: item['secret'] as bool? ?? false,
        preselect: item['preselect'] as bool? ?? false,
        actions: _actions(item['actions']),
      );

  static ModuleDelivery _delivery(Map<String, dynamic> delivery) =>
      ModuleDelivery(
        text: delivery['text'] as String,
        label: delivery['label'] as String,
        secret: delivery['secret'] as bool? ?? false,
        requireAuth: delivery['require_auth'] as bool? ?? false,
        recordHistory: delivery['record_history'] as bool? ?? false,
        cursorFromEnd: delivery['cursor_graphemes_from_end'] as int?,
      );

  static Iterable<Map<String, dynamic>> _maps(Object? value) =>
      (value as List<dynamic>? ?? const []).cast<Map<String, dynamic>>();

  static List<String> _strings(Object? value) =>
      List.unmodifiable((value as List<dynamic>? ?? const []).cast<String>());
}
