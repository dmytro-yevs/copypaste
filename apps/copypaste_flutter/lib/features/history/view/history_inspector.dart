import 'dart:async';
import 'dart:math' as math;

import 'package:copypaste_flutter/app/theme/app_motion.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/app/theme/app_toast.dart';
import 'package:copypaste_flutter/app/theme/app_tokens.dart';
import 'package:copypaste_flutter/features/devices/device_label.dart';
import 'package:copypaste_flutter/features/history/controller/history_controller.dart';
import 'package:copypaste_flutter/features/history/models/history_models.dart';
import 'package:copypaste_flutter/features/history/presentation/history_code_highlighter.dart';
import 'package:copypaste_flutter/features/history/presentation/history_clip_presentation.dart';
import 'package:copypaste_flutter/features/history/presentation/source_app_label.dart';
import 'package:copypaste_flutter/shared/inspector_table.dart';
import 'package:copypaste_flutter/shared/state_view.dart';
import 'package:copypaste_flutter/shared/system_date_time.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../presentation/history_color_swatch.dart';
import 'history_delete_dialog.dart';
import 'history_ocr_dialog.dart';

class HistoryInspector extends StatelessWidget {
  const HistoryInspector({
    super.key,
    required this.controller,
    required this.inDrawer,
    this.onClose,
    this.showActions = true,
    this.compact = false,
  });

  final HistoryController controller;
  final bool inDrawer;
  final VoidCallback? onClose;
  final bool showActions;
  final bool compact;

  @override
  Widget build(BuildContext context) {
    final clip = controller.selectedClip;
    if (clip == null) {
      return const StateView.empty(
        title: 'Select a clip',
        message: 'Its full content and actions appear here.',
      );
    }
    final theme = Theme.of(context);
    final body = (clip.body ?? clip.preview).isEmpty
        ? 'No preview is available.'
        : clip.body ?? clip.preview;
    final highlighted = switch (clip.contentKind) {
      HistoryClipKind.code => HistoryCodeHighlighter.highlight(body, theme),
      HistoryClipKind.json => HistoryCodeHighlighter.highlight(
        body,
        theme,
        json: true,
      ),
      _ => null,
    };
    final content = Card(
      theme: compact ? AppTheme.clipboardInspectorCardTheme : null,
      key: ValueKey<String>(
        inDrawer ? 'history-detail-drawer-card' : 'history-detail-inspector',
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsets.all(AppSpacing.xs),
            child: Row(
              key: const ValueKey<String>('history-detail-heading'),
              children: [
                SizedBox.square(
                  key: const ValueKey<String>('history-detail-kind-icon'),
                  dimension: inDrawer || compact
                      ? AppControlSize.compact
                      : AppControlSize.large,
                  child: Card(
                    theme: CardTheme(
                      padding: EdgeInsets.zero,
                      filled: true,
                      fillColor: theme.colorScheme.secondary,
                      borderRadius: theme.borderRadiusMd,
                      borderWidth: AppSpacing.zero,
                    ),
                    child: Center(
                      child: Icon(
                        HistoryClipPresentation.icon(clip.contentKind),
                        size: AppIconSize.sm,
                      ),
                    ),
                  ),
                ),
                Gap(inDrawer || compact ? AppSpacing.sm : AppSpacing.md),
                Expanded(
                  child: compact
                      ? Text(
                          clip.contentKind.label,
                          style: AppTheme.clipboardMenuTextStyle(context),
                        )
                      : inDrawer
                      ? Text(
                          clip.contentKind.label,
                          style: theme.typography.small.merge(
                            theme.typography.semiBold,
                          ),
                        )
                      : Text(clip.contentKind.label).h4(),
                ),
                if (highlighted != null) ...[
                  const Gap(AppSpacing.sm),
                  SecondaryBadge(child: Text(highlighted.language)),
                ],
                if (!inDrawer) ...[
                  const Gap(AppSpacing.sm),
                  Tooltip(
                    showDuration: AppMotion.resolve(
                      context,
                      AppMotion.standard,
                    ),
                    tooltip: (context) => const TooltipContainer(
                      child: Text('Close clip details'),
                    ),
                    child: Semantics(
                      label: 'Close clip details',
                      button: true,
                      child: Button.ghost(
                        key: const ValueKey<String>(
                          'history-detail-inspector-close',
                        ),
                        style: AppTheme.actionButtonStyle(
                          const ButtonStyle.ghostIcon(),
                        ),
                        onPressed: onClose ?? controller.clearSelection,
                        child: const Icon(LucideIcons.x),
                      ),
                    ),
                  ),
                ],
              ],
            ),
          ),
          Gap(inDrawer ? AppSpacing.sm : AppSpacing.lg),
          Flexible(
            fit: inDrawer || compact ? FlexFit.tight : FlexFit.loose,
            child: _HistoryDetailContent(
              clip: clip,
              controller: controller,
              body: body,
              highlighted: highlighted,
              desktop: !inDrawer,
            ),
          ),
          if (showActions) const Gap(AppSpacing.lg),
          if (showActions)
            Padding(
              padding: const EdgeInsets.fromLTRB(
                AppSpacing.xs,
                AppSpacing.zero,
                AppSpacing.xs,
                AppSpacing.xs,
              ),
              child: LayoutBuilder(
                builder: (context, constraints) {
                  final iconOnly = _actionsNeedIcons(
                    context,
                    clip,
                    constraints.maxWidth,
                  );
                  return Wrap(
                    key: const ValueKey<String>('history-detail-actions'),
                    spacing: AppSpacing.sm,
                    runSpacing: AppSpacing.sm,
                    children: [
                      ButtonGroup(
                        children: [
                          Tooltip(
                            tooltip: (_) =>
                                const TooltipContainer(child: Text('Copy')),
                            child: Semantics(
                              label: iconOnly ? 'Copy' : null,
                              button: true,
                              child: Button.primary(
                                key: const ValueKey<String>(
                                  'history-detail-copy',
                                ),
                                style: AppTheme.actionButtonStyle(
                                  iconOnly
                                      ? const ButtonStyle.primaryIcon()
                                      : const ButtonStyle.primary(),
                                ),
                                onPressed: () =>
                                    _copy(context, plainText: false),
                                leading: iconOnly
                                    ? null
                                    : const Icon(LucideIcons.copy),
                                child: iconOnly
                                    ? const Icon(LucideIcons.copy)
                                    : const Text('Copy'),
                              ),
                            ),
                          ),
                          if (clip.contentKind.isTextual)
                            Semantics(
                              label: 'Copy options',
                              button: true,
                              child: ComponentTheme<SelectTheme>(
                                data: AppTheme.primarySelectTheme(context),
                                child: Select<bool>(
                                  key: const ValueKey<String>(
                                    'history-copy-options',
                                  ),
                                  value: false,
                                  expandIcon: null,
                                  itemBuilder: (context, _) => Icon(
                                    LucideIcons.chevronDown,
                                    color: Theme.of(context)
                                        .colorScheme
                                        .primaryForeground,
                                  ),
                                  onChanged: (plainText) {
                                    if (plainText == true) {
                                      unawaited(
                                        _copy(context, plainText: true),
                                      );
                                    }
                                  },
                                  popup:
                                      const SelectPopup<bool>.noVirtualization(
                                        items: SelectItemList(
                                          children: [
                                            SelectItemButton<bool>(
                                              value: true,
                                              child: Row(
                                                mainAxisSize: MainAxisSize.min,
                                                children: [
                                                  Icon(
                                                    LucideIcons.alignLeft,
                                                    size: AppIconSize.sm,
                                                  ),
                                                  Gap(AppSpacing.sm),
                                                  Text('Copy plain text'),
                                                ],
                                              ),
                                            ),
                                          ],
                                        ),
                                      ).call,
                                ),
                              ),
                            ),
                        ],
                      ),
                      if (controller.canDownloadSelected)
                        Tooltip(
                          tooltip: (_) =>
                              const TooltipContainer(child: Text('Download')),
                          child: Semantics(
                            label: iconOnly ? 'Download' : null,
                            button: true,
                            child: Button.secondary(
                              key: const ValueKey<String>(
                                'history-detail-download',
                              ),
                              style: AppTheme.actionButtonStyle(
                                iconOnly
                                    ? const ButtonStyle.secondaryIcon()
                                    : const ButtonStyle.secondary(),
                              ),
                              onPressed: controller.isDownloadPending(clip.id)
                                  ? null
                                  : () => _download(context),
                              leading: iconOnly
                                  ? null
                                  : const Icon(LucideIcons.download),
                              child: iconOnly
                                  ? const Icon(LucideIcons.download)
                                  : const Text('Download'),
                            ),
                          ),
                        ),
                      Tooltip(
                        tooltip: (_) => TooltipContainer(
                          child: Text(clip.pinned ? 'Pinned' : 'Pin'),
                        ),
                        child: Semantics(
                          label: iconOnly
                              ? (clip.pinned ? 'Pinned' : 'Pin')
                              : null,
                          toggled: clip.pinned,
                          child: Button(
                            key: ValueKey<String>('history-pin-${clip.id}'),
                            style: AppTheme.actionButtonStyle(
                              ButtonStyle(
                                variance: clip.pinned
                                    ? ButtonVariance.secondary
                                    : ButtonVariance.outline,
                                density: iconOnly
                                    ? ButtonDensity.icon
                                    : ButtonDensity.normal,
                              ),
                            ),
                            onPressed: controller.isPinPending(clip.id)
                                ? null
                                : () => controller.togglePin(clip),
                            leading: iconOnly
                                ? null
                                : const Icon(LucideIcons.pin),
                            child: iconOnly
                                ? const Icon(LucideIcons.pin)
                                : Text(clip.pinned ? 'Pinned' : 'Pin'),
                          ),
                        ),
                      ),
                      if (clip.contentKind == HistoryClipKind.image &&
                          controller.ocr?.available == true)
                        Tooltip(
                          tooltip: (_) => const TooltipContainer(
                            child: Text('Recognize image text'),
                          ),
                          child: Semantics(
                            label: 'Recognize image text',
                            button: true,
                            child: Button.secondary(
                              key: const ValueKey<String>('history-ocr'),
                              style: AppTheme.actionButtonStyle(
                                const ButtonStyle.secondaryIcon(),
                              ),
                              onPressed: controller.ocr!.canRun
                                  ? () => showHistoryOcrDialog(
                                      context,
                                      controller: controller.ocr!,
                                      clip: clip,
                                    )
                                  : null,
                              child: const Icon(LucideIcons.scanText),
                            ),
                          ),
                        ),
                      Tooltip(
                        tooltip: (_) =>
                            const TooltipContainer(child: Text('Delete')),
                        child: Semantics(
                          label: iconOnly ? 'Delete' : null,
                          button: true,
                          child: Button.destructive(
                            key: const ValueKey<String>(
                              'history-detail-delete',
                            ),
                            style: AppTheme.actionButtonStyle(
                              iconOnly
                                  ? const ButtonStyle.destructiveIcon()
                                  : const ButtonStyle.destructive(),
                            ),
                            onPressed: () => showHistoryDeleteDialog(
                              context,
                              controller: controller,
                              clipId: clip.id,
                            ),
                            leading: iconOnly
                                ? null
                                : const Icon(LucideIcons.trash2),
                            child: iconOnly
                                ? const Icon(LucideIcons.trash2)
                                : const Text('Delete'),
                          ),
                        ),
                      ),
                    ],
                  );
                },
              ),
            ),
          if (!inDrawer) ...[
            const Gap(AppSpacing.md),
            Padding(
              padding: const EdgeInsets.fromLTRB(
                AppSpacing.xs,
                AppSpacing.zero,
                AppSpacing.xs,
                AppSpacing.xs,
              ),
              child: _HistoryMetadataTable(
                clip: clip,
                controller: controller,
                compact: compact,
              ),
            ),
          ],
        ],
      ),
    );
    if (!inDrawer) {
      return content;
    }
    return Padding(
      padding: const EdgeInsets.fromLTRB(
        AppSpacing.lg,
        AppSpacing.zero,
        AppSpacing.lg,
        AppSpacing.lg,
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Align(
            alignment: Alignment.centerRight,
            child: Button.ghost(
              key: const ValueKey<String>('history-detail-drawer-close'),
              style: AppTheme.actionButtonStyle(const ButtonStyle.ghostIcon()),
              onPressed: () => closeDrawer(context),
              child: const Icon(LucideIcons.x),
            ),
          ),
          const Gap(AppSpacing.xs),
          Expanded(child: content),
        ],
      ),
    );
  }

  bool _actionsNeedIcons(
    BuildContext context,
    HistoryClip clip,
    double availableWidth,
  ) {
    final actions = <(ButtonStyle, String?)>[
      (const ButtonStyle.primary(), 'Copy'),
      if (controller.canDownloadSelected)
        (const ButtonStyle.secondary(), 'Download'),
      (
        clip.pinned
            ? const ButtonStyle.secondary()
            : const ButtonStyle.outline(),
        clip.pinned ? 'Pinned' : 'Pin',
      ),
      if (clip.contentKind == HistoryClipKind.image &&
          controller.ocr?.available == true)
        (const ButtonStyle.secondaryIcon(), null),
      (const ButtonStyle.destructive(), 'Delete'),
    ];
    final theme = Theme.of(context);
    double widthOf(ButtonStyle buttonStyle, String? label) {
      final style = AppTheme.actionButtonStyle(buttonStyle);
      final iconWidth =
          style.iconTheme(context, const {}).size ?? AppIconSize.sm;
      final padding = style
          .padding(context, const {})
          .resolve(Directionality.of(context));
      if (label == null) return padding.horizontal + iconWidth;
      final painter = TextPainter(
        text: TextSpan(
          text: label,
          style: DefaultTextStyle.of(context).style
              .merge(style.textStyle(context, const {})),
        ),
        textDirection: Directionality.of(context),
        textScaler: MediaQuery.textScalerOf(context),
        maxLines: 1,
      )..layout();
      final width =
          padding.horizontal +
          iconWidth +
          theme.density.baseGap * theme.scaling +
          painter.width.ceilToDouble();
      painter.dispose();
      return width;
    }

    var requiredWidth = (actions.length - 1) * AppSpacing.sm;
    for (final (style, label) in actions) {
      requiredWidth += widthOf(style, label);
    }
    // The attached Copy select occupies space inside the first action group.
    if (clip.contentKind.isTextual) {
      requiredWidth += widthOf(const ButtonStyle.primaryIcon(), null);
    }
    return requiredWidth > availableWidth;
  }

  Future<void> _copy(BuildContext context, {required bool plainText}) async {
    final copied = await controller.copySelected(plainText: plainText);
    if (!copied || !context.mounted) return;
    AppToast.show(
      context,
      title: 'Copied',
      message: plainText ? 'Plain text copied.' : 'Clip copied.',
      tone: AppToastTone.success,
    );
  }

  Future<void> _download(BuildContext context) async {
    final result = await controller.downloadSelected();
    if (result != HistoryFileDownloadResult.saved || !context.mounted) return;
    AppToast.show(
      context,
      title: 'Downloaded',
      message: 'File saved.',
      tone: AppToastTone.success,
    );
  }
}

class _HistoryDetailContent extends StatelessWidget {
  const _HistoryDetailContent({
    required this.clip,
    required this.controller,
    required this.body,
    required this.highlighted,
    required this.desktop,
  });

  final HistoryClip clip;
  final HistoryController controller;
  final String body;
  final HighlightedHistoryCode? highlighted;
  final bool desktop;

  @override
  Widget build(BuildContext context) {
    if (!desktop) {
      if (clip.contentKind == HistoryClipKind.image) {
        return Padding(
          key: const ValueKey<String>('history-detail-image-fit-content'),
          padding: const EdgeInsets.symmetric(horizontal: AppSpacing.xs),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Expanded(
                child: _DetailImagePreview(clip: clip, controller: controller),
              ),
              const Gap(AppSpacing.lg),
              Flexible(
                child: SingleChildScrollView(
                  primary: false,
                  key: const ValueKey<String>('history-detail-scroll-metadata'),
                  child: _HistoryMetadataTable(
                    clip: clip,
                    controller: controller,
                  ),
                ),
              ),
            ],
          ),
        );
      }
      return SingleChildScrollView(
        primary: false,
        key: const ValueKey<String>('history-detail-scroll-content'),
        padding: const EdgeInsets.symmetric(horizontal: AppSpacing.xs),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            _body(context),
            const Gap(AppSpacing.lg),
            _HistoryMetadataTable(clip: clip, controller: controller),
          ],
        ),
      );
    }

    return LayoutBuilder(
      builder: (context, constraints) {
        final content = clip.contentKind == HistoryClipKind.image
            ? _DetailImagePreview(
                clip: clip,
                controller: controller,
                maxHeight: constraints.maxHeight,
              )
            : SizedBox(width: double.infinity, child: _body(context));
        return SingleChildScrollView(
          primary: false,
          key: const ValueKey<String>('history-detail-scroll-content'),
          padding: const EdgeInsets.symmetric(horizontal: AppSpacing.xs),
          child: content,
        );
      },
    );
  }

  Widget _body(BuildContext context) {
    if (clip.contentKind == HistoryClipKind.color && clip.colorRgba != null) {
      return Row(
        children: [
          HistoryColorSwatch(rgba: clip.colorRgba!, size: 48),
          const Gap(AppSpacing.md),
          Expanded(
            child: SelectableText(
              body,
              style: Theme.of(context).typography.mono,
            ),
          ),
        ],
      );
    }
    if (highlighted != null) {
      return SelectableText.rich(highlighted!.span);
    }
    if (clip.contentKind != HistoryClipKind.image) {
      return SelectableText(
        body,
        style:
            clip.contentKind == HistoryClipKind.path ||
                clip.contentKind == HistoryClipKind.file
            ? Theme.of(context).typography.mono
            : null,
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _DetailImagePreview(clip: clip, controller: controller),
        const Gap(AppSpacing.lg),
        SelectableText(body),
      ],
    );
  }
}

class _HistoryMetadataTable extends StatelessWidget {
  const _HistoryMetadataTable({
    required this.clip,
    required this.controller,
    this.compact = false,
  });

  final HistoryClip clip;
  final HistoryController controller;
  final bool compact;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final rows = _historyMetadataRows(context, clip);
    final textStyle = AppTheme.inspectorTextStyle(context, compact: compact);
    return InspectorTable(
      tableKey: const ValueKey<String>('history-detail-metadata'),
      compact: compact,
      rows: [
        for (final row in rows)
          (
            label: row.label,
            value: _historyMetadataHasIdentity(row)
                ? _historyMetadataIdentityLabel(
                    row: row,
                    clip: clip,
                    controller: controller,
                    style: textStyle,
                  )
                : SelectableText(
                    row.value,
                    style: textStyle.copyWith(
                      color: row.warning ? theme.colorScheme.destructive : null,
                    ),
                  ),
          ),
      ],
    );
  }
}

typedef _HistoryMetadataRow = ({String label, String value, bool warning});

bool _historyMetadataHasIdentity(_HistoryMetadataRow row) =>
    row.label == 'Observed app' || row.label == 'Device';

Widget _historyMetadataIdentityLabel({
  required _HistoryMetadataRow row,
  required HistoryClip clip,
  required HistoryController controller,
  required TextStyle style,
}) {
  return switch (row.label) {
    'Observed app' => SourceAppLabel(
      key: const ValueKey<String>('history-detail-source-app'),
      name: row.value,
      icon: controller.requestSourceIcon(clip.sourceAppIconId),
      style: style,
    ),
    'Device' => DeviceLabel(
      key: const ValueKey<String>('history-detail-device'),
      name: row.value,
      deviceClass: clip.originDeviceClass,
      style: style,
    ),
    _ => throw StateError('Metadata row has no identity presentation.'),
  };
}

List<_HistoryMetadataRow> _historyMetadataRows(
  BuildContext context,
  HistoryClip clip,
) {
  final type = clip.file?.mimeType ?? clip.contentType;
  final sizeBytes = clip.image?.sizeBytes ?? clip.file?.sizeBytes;
  return [
    (
      label: 'Captured',
      value: formatSystemDateTime(context, clip.createdAt),
      warning: false,
    ),
    if (clip.sourceApp != null)
      (label: 'Observed app', value: clip.sourceApp!, warning: false),
    if (clip.origin != null)
      (label: 'Device', value: clip.origin!, warning: false),
    (label: 'Clip type', value: clip.contentKind.label, warning: false),
    (label: 'Type', value: type, warning: false),
    if (clip.file?.sourceReference != null)
      (
        label: clip.file!.sourceReference!.startsWith('content://')
            ? 'URI'
            : 'Path',
        value: clip.file!.sourceReference!,
        warning: false,
      ),
    if (clip.file?.name != null)
      (label: 'File', value: clip.file!.name!, warning: false),
    if (clip.image != null)
      (
        label: 'Resolution',
        value: '${clip.image!.width} × ${clip.image!.height}',
        warning: false,
      ),
    if (sizeBytes != null)
      (label: 'Size', value: _formatBytes(sizeBytes), warning: false),
    if ((clip.file?.fileCount ?? 0) > 1)
      (label: 'Files', value: '${clip.file!.fileCount}', warning: false),
    if (clip.tooLargeToSync)
      (label: 'Sync', value: 'Too large to sync', warning: true),
  ];
}

class _DetailImagePreview extends StatelessWidget {
  const _DetailImagePreview({
    required this.clip,
    required this.controller,
    this.maxHeight,
  });

  final HistoryClip clip;
  final HistoryController controller;
  final double? maxHeight;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) => FutureBuilder<HistoryImagePreview?>(
        future: controller.requestImagePreview(clip.id, maxEdge: 1024),
        builder: (context, snapshot) {
          final preview = snapshot.data;
          if (preview == null) {
            return const Align(
              alignment: Alignment.topLeft,
              child: SizedBox.square(
                dimension: AppIconSize.hero,
                child: Center(
                  child: Icon(LucideIcons.image, size: AppIconSize.hero),
                ),
              ),
            );
          }
          if (preview.width <= 0 || preview.height <= 0) {
            return const Align(
              alignment: Alignment.topLeft,
              child: Icon(LucideIcons.imageOff, size: AppIconSize.hero),
            );
          }
          final sourceWidth = preview.width.toDouble();
          final sourceHeight = preview.height.toDouble();
          final availableWidth = constraints.hasBoundedWidth
              ? constraints.maxWidth
              : sourceWidth;
          final availableHeight =
              maxHeight ??
              (constraints.hasBoundedHeight
                  ? constraints.maxHeight
                  : sourceHeight);
          final scale = math.min(
            1.0,
            math.min(
              availableWidth / sourceWidth,
              availableHeight / sourceHeight,
            ),
          );
          return Align(
            alignment: Alignment.topLeft,
            child: SizedBox(
              key: const ValueKey<String>('history-detail-image-viewport'),
              width: sourceWidth * scale,
              height: sourceHeight * scale,
              child: Image.memory(
                preview.bytes,
                key: ValueKey<String>('history-detail-image-${clip.id}'),
                fit: BoxFit.contain,
                alignment: Alignment.topLeft,
                errorBuilder: (context, error, stackTrace) => const Center(
                  child: Icon(LucideIcons.imageOff, size: AppIconSize.hero),
                ),
              ),
            ),
          );
        },
      ),
    );
  }
}

String _formatBytes(int bytes) {
  if (bytes < 1024) return '$bytes B';
  if (bytes < 1024 * 1024) return '${(bytes / 1024).toStringAsFixed(1)} KB';
  return '${(bytes / (1024 * 1024)).toStringAsFixed(1)} MB';
}
