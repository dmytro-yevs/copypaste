import 'dart:async';

import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_motion.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../app/theme/app_tooltip.dart';
import '../../../l10n/l10n.dart';
import '../../settings/view/settings_section.dart';
import '../models/module_view.dart';
import 'module_icons.dart';

/// Renders a module view: rows and text blocks in settings groups, and
/// library controls between them. Actions reach [onAction] with any
/// host-provided arguments already added.
class ModuleViewSections extends StatefulWidget {
  const ModuleViewSections({
    super.key,
    required this.view,
    required this.onAction,
    required this.onCopy,
    this.enabled = true,
    this.formBuilder,
    this.preferenceBuilder,
  });

  final ModuleView view;
  final ValueChanged<ModuleViewAction> onAction;
  final Future<void> Function(String text) onCopy;
  final bool enabled;
  final Widget Function(BuildContext context, ModuleViewForm form)? formBuilder;
  final Widget? Function(BuildContext context, String id)? preferenceBuilder;

  @override
  State<ModuleViewSections> createState() => _ModuleViewSectionsState();
}

class _ModuleViewSectionsState extends State<ModuleViewSections> {
  static const _searchDelay = Duration(milliseconds: 250);
  static const _masked = '••••••••';

  final _selection = <int, Set<String>>{};
  final _sortable = <String, SortableData<String>>{};
  Timer? _search;

  @override
  void didUpdateWidget(ModuleViewSections oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (identical(oldWidget.view, widget.view)) return;
    // Rows that left the view leave the selection.
    for (final (index, section) in widget.view.sections.indexed) {
      final ids = _rowIds(section).toSet();
      _selection[index]?.retainWhere(ids.contains);
    }
    _selection.removeWhere(
      (index, ids) => index >= widget.view.sections.length || ids.isEmpty,
    );
  }

  @override
  void dispose() {
    _search?.cancel();
    super.dispose();
  }

  static Iterable<String> _rowIds(ModuleViewSection section) => [
    for (final item in section.items)
      if (item case ModuleViewRow(:final id?)) id,
  ];

  void _act(ModuleViewAction action, [Map<String, Object> extra = const {}]) {
    if (!widget.enabled) return;
    widget.onAction(extra.isEmpty ? action : action.withArguments(extra));
  }

  void _searchChanged(ModuleViewSearch search, String query) {
    _search?.cancel();
    _search = Timer(_searchDelay, () {
      if (!mounted) return;
      widget.onAction(search.action.withArguments({'query': query}));
    });
  }

  void _toggle(int section, String id) => setState(() {
    final selected = _selection.putIfAbsent(section, () => <String>{});
    if (!selected.remove(id)) selected.add(id);
  });

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      for (final (index, section) in widget.view.sections.indexed) ...[
        if (index > 0) const Gap(AppSpacing.lg),
        if (section.title case final title?) ...[
          Text(title).semiBold(),
          const Gap(AppSpacing.sm),
        ],
        ..._section(context, index, section),
      ],
    ],
  );

  /// Rows, blocks and preferences share cards; other items stand between
  /// them.
  List<Widget> _section(
    BuildContext context,
    int index,
    ModuleViewSection section,
  ) {
    final children = <Widget>[];
    var card = <Widget>[];
    void add(Widget child) {
      if (children.isNotEmpty) children.add(const Gap(AppSpacing.md));
      children.add(child);
    }

    void closeCard() {
      if (card.isEmpty) return;
      final group = SettingsGroup(children: card);
      add(
        section.reorder == null
            ? group
            : SortableLayer(
                dropDuration: AppMotion.resolve(context, AppMotion.standard),
                dropCurve: AppMotion.standardCurve,
                child: group,
              ),
      );
      card = [];
    }

    final selected = _selection[index] ?? const <String>{};
    if (selected.isNotEmpty && section.bulkActions.isNotEmpty) {
      add(_bulkBar(context, index, section, selected));
    }
    final rowIds = _rowIds(section).toList();
    for (final item in section.items) {
      switch (item) {
        case ModuleViewRow():
          card.add(_row(context, index, section, item, rowIds));
        case ModuleViewBlock():
          card.add(_TextBlock(block: item, onCopy: widget.onCopy));
        case ModuleViewPreference(:final id):
          if (widget.preferenceBuilder?.call(context, id) case final child?) {
            card.add(child);
          }
        case ModuleViewSearch():
          closeCard();
          add(_searchField(context, index, item));
        case ModuleViewFilter():
          closeCard();
          add(_filter(item));
        case ModuleViewToolbar(:final actions):
          closeCard();
          add(
            Wrap(
              spacing: AppSpacing.sm,
              runSpacing: AppSpacing.sm,
              children: [for (final action in actions) _button(action)],
            ),
          );
        case ModuleViewNotice():
          closeCard();
          add(_notice(context, item));
        case ModuleViewForm():
          closeCard();
          if (widget.formBuilder case final builder?) {
            add(builder(context, item));
          }
      }
    }
    closeCard();
    if (section.more case final more?) {
      add(
        Button.ghost(
          key: ValueKey('module-view-more-$index'),
          style: AppTheme.actionButtonStyle(const ButtonStyle.ghost()),
          onPressed: widget.enabled ? () => _act(more) : null,
          child: Text(context.l10n.historyLoadMore),
        ),
      );
    }
    return children;
  }

  Widget _bulkBar(
    BuildContext context,
    int index,
    ModuleViewSection section,
    Set<String> selected,
  ) => Wrap(
    key: ValueKey('module-view-bulk-$index'),
    spacing: AppSpacing.sm,
    runSpacing: AppSpacing.sm,
    children: [
      for (final action in section.bulkActions)
        _button(action, {'ids': selected.toList(growable: false)}),
      Button.ghost(
        style: AppTheme.actionButtonStyle(const ButtonStyle.ghost()),
        onPressed: () => setState(() => _selection.remove(index)),
        child: Text(context.l10n.commonClear),
      ),
    ],
  );

  Widget _button(
    ModuleViewAction action, [
    Map<String, Object> extra = const {},
  ]) => Button(
    key: ValueKey('module-view-action-${action.command}-${action.title}'),
    style: AppTheme.actionButtonStyle(
      action.destructive
          ? const ButtonStyle.destructive()
          : const ButtonStyle.secondary(),
    ),
    onPressed: widget.enabled ? () => _act(action, extra) : null,
    leading: action.icon == null
        ? null
        : AppIconFrame(child: Icon(action.icon!.data)),
    child: Text(action.title),
  );

  Widget _searchField(BuildContext context, int index, ModuleViewSearch item) =>
      TextField(
        key: ValueKey('module-view-search-$index'),
        initialValue: item.query,
        placeholder: Text(item.placeholder),
        onChanged: (query) => _searchChanged(item, query),
        decoration: AppTheme.searchFieldDecoration(context),
        skipInputFeatureFocusTraversal: true,
        features: const [
          InputFeature.leading(AppIconFrame(child: Icon(LucideIcons.search))),
          InputFeature.clear(),
        ],
      );

  Widget _filter(ModuleViewFilter item) => Wrap(
    spacing: AppSpacing.xs,
    runSpacing: AppSpacing.xs,
    children: [
      for (final option in item.options)
        Button(
          key: ValueKey('module-view-filter-${option.id}'),
          style: AppTheme.controlButtonStyle(
            option.id == item.selected
                ? const ButtonStyle.secondary()
                : const ButtonStyle.ghost(),
          ),
          onPressed: widget.enabled && option.id != item.selected
              ? () => _act(item.action, {'value': option.id})
              : null,
          leading: option.icon == null
              ? null
              : AppIconFrame(child: Icon(option.icon!.data)),
          trailing: option.count == null
              ? null
              : Text('${option.count}').muted().small(),
          child: Text(option.title),
        ),
    ],
  );

  Widget _notice(BuildContext context, ModuleViewNotice item) =>
      switch (item.tone) {
        ModuleNoticeTone.error => Alert.destructive(
          leading: const AppIconFrame(child: Icon(LucideIcons.circleAlert)),
          content: Text(item.text),
        ),
        ModuleNoticeTone.warning => Alert(
          leading: AppTheme.attentionAlertIcon(context),
          content: Text(item.text),
        ),
        ModuleNoticeTone.info => Alert(
          leading: const AppIconFrame(child: Icon(LucideIcons.info)),
          content: Text(item.text),
        ),
      };

  Widget _row(
    BuildContext context,
    int index,
    ModuleViewSection section,
    ModuleViewRow row,
    List<String> rowIds,
  ) {
    final simple =
        row.id == null &&
        row.open == null &&
        row.icon == null &&
        row.badges.isEmpty &&
        !row.secret;
    if (simple) {
      return SettingRow(
        title: row.title,
        description: row.subtitle,
        trailing: row.actions.isEmpty
            ? null
            : Wrap(
                spacing: AppSpacing.sm,
                runSpacing: AppSpacing.sm,
                children: [for (final action in row.actions) _button(action)],
              ),
      );
    }
    final id = row.id;
    final selectable = id != null && section.bulkActions.isNotEmpty;
    final selected = selectable && (_selection[index]?.contains(id) ?? false);
    final subtitle = row.secret
        ? _masked
        : row.subtitle?.trim().split('\n').first;
    final content = Padding(
      padding: AppTheme.settingsRowPadding,
      child: Row(
        children: [
          if (id != null && section.reorder != null)
            SortableDragHandle(
              child: _iconAction(
                context.l10n.rulesReorder,
                LucideIcons.gripVertical,
                () {},
              ),
            ),
          if (selectable)
            Checkbox(
              key: ValueKey('module-view-select-$id'),
              state: selected ? CheckboxState.checked : CheckboxState.unchecked,
              onChanged: widget.enabled ? (_) => _toggle(index, id) : null,
            ),
          Expanded(
            child: Button(
              key: ValueKey('module-view-row-${id ?? row.title}'),
              style: AppTheme.historyClipButtonStyle(selected: selected),
              alignment: Alignment.centerLeft,
              onPressed: !widget.enabled
                  ? null
                  : row.open != null
                  ? () => _act(row.open!)
                  : selectable
                  ? () => _toggle(index, id)
                  : null,
              leading: row.icon == null
                  ? null
                  : AppIconFrame(child: Icon(row.icon!.data)),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisSize: MainAxisSize.min,
                children: [
                  Wrap(
                    spacing: AppSpacing.xs,
                    runSpacing: AppSpacing.xs,
                    crossAxisAlignment: WrapCrossAlignment.center,
                    children: [
                      Text(
                        row.title,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ).medium(),
                      for (final badge in row.badges)
                        SecondaryBadge(child: Text(badge)),
                    ],
                  ),
                  if (subtitle != null && subtitle.isNotEmpty) ...[
                    const Gap(AppSpacing.xxs),
                    Text(
                      subtitle,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ).small().muted(),
                  ],
                ],
              ),
            ),
          ),
          for (final action in row.actions)
            if (action.icon case final icon?)
              _iconAction(
                action.title,
                icon.data,
                widget.enabled ? () => _act(action) : null,
                key: ValueKey(
                  'module-view-action-${action.command}-${action.title}',
                ),
              )
            else ...[
              const Gap(AppSpacing.sm),
              _button(action),
            ],
        ],
      ),
    );
    final reorder = section.reorder;
    if (id == null || reorder == null) return content;
    bool accepts(SortableData<String> data) => data.data != id;
    void move(String dragged, String before) {
      if (dragged != before) {
        _act(reorder, {'id': dragged, 'before': before});
      }
    }

    final position = rowIds.indexOf(id);
    final next = position + 1 < rowIds.length ? rowIds[position + 1] : '';
    return Sortable<String>(
      key: ValueKey('module-view-sortable-$id'),
      data: _sortable.putIfAbsent(id, () => SortableData(id)),
      enabled: false,
      canAcceptTop: accepts,
      canAcceptBottom: accepts,
      onAcceptTop: (data) => move(data.data, id),
      onAcceptBottom: (data) => move(data.data, next),
      placeholder: const SizedBox(height: AppControlSize.regular),
      child: content,
    );
  }

  Widget _iconAction(
    String label,
    IconData icon,
    VoidCallback? onPressed, {
    Key? key,
  }) => AppTooltip(
    tooltip: (context) => TooltipContainer(child: Text(label)),
    child: Button.ghost(
      key: key,
      style: AppTheme.controlButtonStyle(const ButtonStyle.ghostIcon()),
      onPressed: onPressed,
      child: Semantics(
        label: label,
        child: AppIconFrame(child: Icon(icon)),
      ),
    ),
  );
}

class _TextBlock extends StatefulWidget {
  const _TextBlock({required this.block, required this.onCopy});

  final ModuleViewBlock block;
  final Future<void> Function(String text) onCopy;

  @override
  State<_TextBlock> createState() => _TextBlockState();
}

class _TextBlockState extends State<_TextBlock> {
  bool _copied = false;

  Future<void> _copy() async {
    await widget.onCopy(widget.block.text);
    if (mounted) setState(() => _copied = true);
  }

  @override
  Widget build(BuildContext context) => Padding(
    padding: AppTheme.settingsRowPadding,
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (widget.block.label case final label?) ...[
          Text(label).small(),
          const Gap(AppSpacing.xs),
        ],
        CodeSnippet(
          code: SelectableText(widget.block.text),
          actions: [
            AppTooltip(
              tooltip: (context) =>
                  TooltipContainer(child: Text(context.l10n.commonCopy)),
              child: Button.ghost(
                style: AppTheme.controlButtonStyle(
                  const ButtonStyle.ghostIcon(),
                ),
                onPressed: () => unawaited(_copy()),
                child: Semantics(
                  label: context.l10n.commonCopy,
                  child: AppIconFrame(
                    child: Icon(
                      _copied ? LucideIcons.copyCheck : LucideIcons.copy,
                    ),
                  ),
                ),
              ),
            ),
          ],
        ),
      ],
    ),
  );
}
