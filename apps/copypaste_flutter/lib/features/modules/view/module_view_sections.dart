import 'dart:async';

import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../../settings/view/settings_section.dart';
import '../models/module_view.dart';

/// Renders a module view: each section is a settings group of rows, whose
/// actions are buttons, and copyable text blocks.
class ModuleViewSections extends StatelessWidget {
  const ModuleViewSections({
    super.key,
    required this.view,
    required this.onAction,
    required this.onCopy,
    this.enabled = true,
  });

  final ModuleView view;
  final ValueChanged<ModuleViewAction> onAction;
  final Future<void> Function(String text) onCopy;
  final bool enabled;

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      for (final (index, section) in view.sections.indexed) ...[
        if (index > 0) const Gap(AppSpacing.lg),
        if (section.title case final title?) ...[
          Text(title).semiBold(),
          const Gap(AppSpacing.sm),
        ],
        if (section.items.isNotEmpty)
          SettingsGroup(
            children: [
              for (final item in section.items)
                switch (item) {
                  ModuleViewRow() => SettingRow(
                    title: item.title,
                    description: item.subtitle,
                    trailing: item.actions.isEmpty
                        ? null
                        : Wrap(
                            spacing: AppSpacing.sm,
                            runSpacing: AppSpacing.sm,
                            children: [
                              for (final action in item.actions)
                                Button(
                                  key: ValueKey(
                                    'module-view-action-${action.command}-${action.title}',
                                  ),
                                  style: AppTheme.actionButtonStyle(
                                    action.destructive
                                        ? const ButtonStyle.destructive()
                                        : const ButtonStyle.secondary(),
                                  ),
                                  onPressed: enabled
                                      ? () => onAction(action)
                                      : null,
                                  child: Text(action.title),
                                ),
                            ],
                          ),
                  ),
                  ModuleViewBlock() => _TextBlock(block: item, onCopy: onCopy),
                },
            ],
          ),
      ],
    ],
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
            Tooltip(
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
