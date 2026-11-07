import '../../../app/theme/app_filter_indicator.dart';
import 'dart:async';

import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';
import '../controller/history_controller.dart';
import 'history_tags_dialog.dart';

/// Uses the same select trigger, popup and selection marks as History facets.
class HistoryTagsFilter extends StatelessWidget {
  const HistoryTagsFilter({
    super.key,
    required this.controller,
    this.compact = true,
  });
  final HistoryController controller;
  final bool compact;

  static String selectionLabel(HistoryController controller) {
    final query = controller.window.query;
    final strings = AppStrings.current;
    if (query.tagIds.isEmpty) return strings.historyTagsFilterAll;
    if (query.tagIds.length > 1) {
      return strings.historyTagsFilterCount(query.tagIds.length);
    }
    for (final tag in controller.tags.tags) {
      if (tag.id == query.tagIds.single) return tag.name;
    }
    return strings.historyTags;
  }

  static const _all = ':all';

  @override
  Widget build(BuildContext context) {
    final query = controller.window.query;
    final tags = controller.tags.tags;
    final label = selectionLabel(controller);
    final selected = query.tagIds.toSet();
    final extent = AppTheme.controlHeight(
      context,
      minimum: AppControlSize.large,
    );
    return AppFilterIndicator(
      active: query.hasTagFilter,
      tooltip: context.l10n.historyTagsFilterTooltip(label),
      child: OverlayAnchor(
        anchor: ('history-tags-filter', controller),
        child: ComponentTheme<SelectTheme>(
          data: AppTheme.selectTheme(
            context,
            variant: compact
                ? AppSelectVariant.compactToolbar
                : AppSelectVariant.toolbar,
            anchor: LinkedAnchor((
              'history-tags-filter',
              controller,
            ), registry: OverlayAnchorRegistry.of(context)),
            labels: [
              label,
              context.l10n.historyTagsFilterAll,
              context.l10n.historyManageTags,
              ...tags.map((tag) => tag.name),
            ],
          ),
          child: Select<Set<String>>(
            key: const ValueKey('history-tags-filter'),
            filled: true,
            enabled: controller.tags.available,
            constraints: compact
                ? BoxConstraints.tightFor(width: extent, height: extent)
                : null,
            value: selected,
            autoClosePopover: false,
            canUnselect: true,
            expandIcon: compact ? null : const SelectExpandIcon(),
            itemBuilder: (context, _) => Semantics(
              label: context.l10n.historyTagsFilterSemantics,
              child: compact
                  ? const AppIconFrame(
                      size: AppIconSize.md,
                      child: Icon(LucideIcons.tags, size: AppIconSize.md),
                    )
                  : Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        const AppIconFrame(
                          size: AppIconSize.sm,
                          child: Icon(LucideIcons.tags, size: AppIconSize.sm),
                        ),
                        const Gap(AppSpacing.sm),
                        Text(label, maxLines: 1),
                      ],
                    ),
            ),
            valueSelectionPredicate: (value, item) => item == _all
                ? value?.isEmpty ?? true
                : value?.contains(item) ?? false,
            valueSelectionHandler: (value, item, selected) {
              if (item == _all) return <String>{};
              final next = {...?value};
              if (item is String) {
                selected ? next.add(item) : next.remove(item);
              }
              return next;
            },
            onChanged: (value) => unawaited(
              controller.window.updateQuery(
                controller.window.query.copyWith(
                  tagIds: value?.toList() ?? const [],
                  untaggedOnly: false,
                ),
              ),
            ),
            popup: SelectPopup<String>(
              shrinkWrap: true,
              items: SelectItemList(
                children: [
                  SelectItemButton<String>(
                    value: _all,
                    child: Text(context.l10n.historyTagsFilterAll),
                  ),
                  if (tags.isNotEmpty) const Divider(),
                  for (final tag in tags)
                    SelectItemButton<String>(
                      value: tag.id,
                      child: Text(tag.name),
                    ),
                  const Divider(),
                  SelectItem(
                    value: null,
                    builder: (popupContext) => Button.ghost(
                      alignment: AlignmentDirectional.centerStart,
                      onPressed: () async {
                        await closeOverlay(popupContext);
                        if (context.mounted) {
                          await showHistoryTagsDialog(
                            context,
                            controller: controller,
                            anchor: ('history-tags-filter', controller),
                          );
                        }
                      },
                      child: Text(context.l10n.historyManageTags),
                    ),
                  ),
                ],
              ),
            ).call,
          ),
        ),
      ),
    );
  }
}
