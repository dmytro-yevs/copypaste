import 'package:copypaste_flutter/app/theme/app_icon_frame.dart';

import '../../transfers/file_operations_view.dart';
import '../../../shared/format_bytes.dart';

import 'dart:async';

import 'history_selectable_text.dart';

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

import '../presentation/secret_spoiler.dart';

import '../presentation/history_color_swatch.dart';
import 'history_delete_dialog.dart';
import 'history_ocr_dialog.dart';

class HistoryInspector extends StatefulWidget {
  const HistoryInspector({
    super.key,
    required this.controller,
    required this.inDrawer,
    this.onClose,
    this.compact = false,
    this.presentationRevision = 0,
  });

  final HistoryController controller;
  final bool inDrawer;
  final VoidCallback? onClose;
  final bool compact;
  final int presentationRevision;

  @override
  State<HistoryInspector> createState() => _HistoryInspectorState();
}

class _HistoryInspectorState extends State<HistoryInspector> {
  final _highlightMemo = HistoryCodeHighlightMemo();
  String? _highlightClipId;

  HistoryController get controller => widget.controller;
  bool get inDrawer => widget.inDrawer;
  VoidCallback? get onClose => widget.onClose;
  bool get compact => widget.compact;
  int get presentationRevision => widget.presentationRevision;

  @override
  void dispose() {
    _highlightMemo.clear();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final clip = controller.selectedClip;
    if (clip?.id != _highlightClipId) {
      _highlightMemo.clear();
      _highlightClipId = clip?.id;
    }
    if (clip == null) {
      _highlightMemo.clear();
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
      HistoryClipKind.code => _highlightMemo.highlight(body, theme),
      HistoryClipKind.json => _highlightMemo.highlight(body, theme, json: true),
      _ => null,
    };
    if (highlighted == null) _highlightMemo.clear();
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
                      child: AppIconFrame(
                        size: AppIconSize.sm,
                        child: Icon(
                          HistoryClipPresentation.icon(clip.contentKind),
                          size: AppIconSize.sm,
                        ),
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
                        child: const AppIconFrame(child: Icon(LucideIcons.x)),
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
              presentationRevision: presentationRevision,
            ),
          ),
          const Gap(AppSpacing.lg),
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
                  constraints.maxWidth,
                );
                final pinIcon = AppIconFrame(
                  child: Icon(
                    clip.pinned ? LucideIcons.pinOff : LucideIcons.pin,
                  ),
                );
                final secretIconOnly =
                    iconOnly ||
                    _actionsNeedIcons(
                      context,
                      constraints.maxWidth,
                      includeSpoilerLabel: true,
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
                              onPressed: () => _copy(context, plainText: false),
                              leading: iconOnly
                                  ? null
                                  : const AppIconFrame(
                                      child: Icon(LucideIcons.copy),
                                    ),
                              child: iconOnly
                                  ? const AppIconFrame(
                                      child: Icon(LucideIcons.copy),
                                    )
                                  : const Text('Copy'),
                            ),
                          ),
                        ),
                        if (clip.contentKind.isTextual)
                          Semantics(
                            label: 'Copy options',
                            button: true,
                            child: ComponentTheme<SelectTheme>(
                              data: AppTheme.selectTheme(
                                context,
                                variant: AppSelectVariant.primaryAction,
                              ),
                              child: Select<bool>(
                                key: const ValueKey<String>(
                                  'history-copy-options',
                                ),
                                value: false,
                                expandIcon: null,
                                itemBuilder: (context, _) => AppIconFrame(
                                  child: Icon(
                                    LucideIcons.chevronDown,
                                    color: Theme.of(
                                      context,
                                    ).colorScheme.primaryForeground,
                                  ),
                                ),
                                onChanged: (plainText) {
                                  if (plainText == true) {
                                    unawaited(_copy(context, plainText: true));
                                  }
                                },
                                popup: const SelectPopup<bool>.noVirtualization(
                                  items: SelectItemList(
                                    children: [
                                      SelectItemButton<bool>(
                                        value: true,
                                        child: Row(
                                          mainAxisSize: MainAxisSize.min,
                                          children: [
                                            AppIconFrame(
                                              size: AppIconSize.sm,
                                              child: Icon(
                                                LucideIcons.alignLeft,
                                                size: AppIconSize.sm,
                                              ),
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
                      _DownloadButton(
                        controller: controller,
                        clip: clip,
                        iconOnly: iconOnly,
                        onDownload: () => _download(context),
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
                          leading: iconOnly ? null : pinIcon,
                          child: iconOnly
                              ? pinIcon
                              : Text(clip.pinned ? 'Pinned' : 'Pin'),
                        ),
                      ),
                    ),
                    Tooltip(
                      tooltip: (_) => TooltipContainer(
                        child: Text(
                          clip.secret ? 'Remove spoiler' : 'Add spoiler',
                        ),
                      ),
                      child: Semantics(
                        label: clip.secret ? 'Remove spoiler' : 'Add spoiler',
                        toggled: clip.secret,
                        child: Button.outline(
                          key: const ValueKey('history-detail-secret'),
                          style: AppTheme.actionButtonStyle(
                            secretIconOnly
                                ? const ButtonStyle.outlineIcon()
                                : const ButtonStyle.outline(),
                          ),
                          onPressed: controller.isSecretPending(clip.id)
                              ? null
                              : () async {
                                  final changed = await controller.toggleSecret(
                                    clip,
                                  );
                                  if (!changed && context.mounted) {
                                    AppToast.show(
                                      context,
                                      title: 'Update failed',
                                      message:
                                          'The clip secrecy could not be updated.',
                                      tone: AppToastTone.error,
                                    );
                                  }
                                },
                          leading: secretIconOnly
                              ? null
                              : AppIconFrame(
                                  child: Icon(
                                    clip.secret
                                        ? LucideIcons.lockOpen
                                        : LucideIcons.lockKeyhole,
                                  ),
                                ),
                          child: secretIconOnly
                              ? AppIconFrame(
                                  child: Icon(
                                    clip.secret
                                        ? LucideIcons.lockOpen
                                        : LucideIcons.lockKeyhole,
                                  ),
                                )
                              : Text(
                                  clip.secret
                                      ? 'Remove spoiler'
                                      : 'Add spoiler',
                                ),
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
                            child: const AppIconFrame(
                              child: Icon(LucideIcons.scanText),
                            ),
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
                          key: const ValueKey<String>('history-detail-delete'),
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
                              : const AppIconFrame(
                                  child: Icon(LucideIcons.trash2),
                                ),
                          child: iconOnly
                              ? const AppIconFrame(
                                  child: Icon(LucideIcons.trash2),
                                )
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
              child: clip.secret
                  ? SecretSpoiler(
                      key: ValueKey(
                        'inspector-metadata-spoiler-${clip.id}-$presentationRevision',
                      ),
                      clipId: clip.id,
                      controller: controller.spoilers,
                      child: _HistoryMetadataTable(
                        clip: clip,
                        controller: controller,
                        compact: compact,
                        revealConfidential: true,
                      ),
                    )
                  : _HistoryMetadataTable(
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
              child: const AppIconFrame(child: Icon(LucideIcons.x)),
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
    double availableWidth, {
    bool includeSpoilerLabel = false,
  }) {
    // Shared actions use the same label budget for every clip type and state.
    // Additional actions can wrap without changing the shared button format.
    final actions = <(ButtonStyle, String?)>[
      (
        includeSpoilerLabel
            ? const ButtonStyle.outline()
            : const ButtonStyle.outlineIcon(),
        includeSpoilerLabel ? 'Remove spoiler' : null,
      ),
      (const ButtonStyle.primary(), 'Copy'),
      (const ButtonStyle.secondary(), 'Pinned'),
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
          style: DefaultTextStyle.of(
            context,
          ).style.merge(style.textStyle(context, const {})),
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

class _DownloadButton extends StatefulWidget {
  const _DownloadButton({
    required this.controller,
    required this.clip,
    required this.iconOnly,
    required this.onDownload,
  });

  final HistoryController controller;
  final HistoryClip clip;
  final bool iconOnly;
  final VoidCallback onDownload;

  @override
  State<_DownloadButton> createState() => _DownloadButtonState();
}

class _DownloadButtonState extends State<_DownloadButton> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) => ListenableBuilder(
    listenable: widget.controller.downloadProgressChanges,
    builder: (context, _) => _buildButton(context),
  );

  Widget _buildButton(BuildContext context) {
    final controller = widget.controller;
    final id = widget.clip.id;
    final pending = controller.isDownloadPending(id);
    final failed = controller.downloadFailedFor(id);
    final cancelling = controller.isDownloadCancelling(id);
    final progress = controller.downloadProgressFor(id);
    final operation = controller.fileOperations?.forClip(id);
    final saved =
        operation?.phase == 'completed' && operation?.localPath != null;
    final percentage = ((progress ?? 0) * 100).floor();
    final showCancel = pending && _hovered && !cancelling;
    final label = cancelling
        ? 'Cancelling…'
        : showCancel
        ? 'Cancel'
        : pending && progress != null
        ? '$percentage%'
        : pending
        ? 'Downloading…'
        : saved
        ? 'Show file'
        : failed
        ? 'Retry'
        : 'Download';
    final indicator = AppIconFrame(
      size: AppIconSize.sm,
      child: CircularProgressIndicator(
        value: progress,
        animated: !MediaQuery.disableAnimationsOf(context),
        theme: const CircularProgressIndicatorTheme(size: AppIconSize.sm),
      ),
    );
    return MouseRegion(
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: Tooltip(
        tooltip: (_) => TooltipContainer(
          child: Text(
            cancelling
                ? 'Cancelling download'
                : pending
                ? 'Downloading $percentage% · Click to cancel'
                : failed
                ? controller.downloadErrorFor(id) ??
                      'Download failed · Click to retry'
                : 'Download',
          ),
        ),
        child: Semantics(
          label: widget.iconOnly
              ? pending
                    ? 'Cancel download, $percentage% complete'
                    : failed
                    ? 'Retry download'
                    : 'Download'
              : null,
          button: true,
          child: SizedBox(
            width: widget.iconOnly ? AppTheme.controlHeight(context) : null,
            height: widget.iconOnly ? AppTheme.controlHeight(context) : null,
            child: Button.secondary(
              key: const ValueKey<String>('history-detail-download'),
              style: AppTheme.actionButtonStyle(
                widget.iconOnly
                    ? const ButtonStyle.secondaryIcon()
                    : const ButtonStyle.secondary(),
              ),
              onPressed: cancelling
                  ? null
                  : pending
                  ? () => controller.cancelDownload(id)
                  : saved
                  ? () => controller.fileOperations!.openFile(operation!)
                  : widget.onDownload,
              leading: widget.iconOnly
                  ? null
                  : showCancel
                  ? const AppIconFrame(child: Icon(LucideIcons.x))
                  : pending
                  ? indicator
                  : failed
                  ? const AppIconFrame(child: Icon(LucideIcons.rotateCcw))
                  : const AppIconFrame(child: Icon(LucideIcons.download)),
              child: widget.iconOnly
                  ? showCancel
                        ? const AppIconFrame(child: Icon(LucideIcons.x))
                        : pending
                        ? Stack(
                            alignment: Alignment.center,
                            children: [
                              AppIconFrame(
                                size: AppIconSize.lg,
                                child: CircularProgressIndicator(
                                  value: progress,
                                  animated: !MediaQuery.disableAnimationsOf(
                                    context,
                                  ),
                                  theme: const CircularProgressIndicatorTheme(
                                    size: AppIconSize.lg,
                                  ),
                                ),
                              ),
                              if (progress != null)
                                SizedBox(
                                  width: AppIconSize.lg,
                                  child: FittedBox(child: Text('$percentage%')),
                                ),
                            ],
                          )
                        : failed
                        ? const AppIconFrame(child: Icon(LucideIcons.rotateCcw))
                        : const AppIconFrame(child: Icon(LucideIcons.download))
                  : Text(label),
            ),
          ),
        ),
      ),
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
    required this.presentationRevision,
  });

  final HistoryClip clip;
  final HistoryController controller;
  final String body;
  final HighlightedHistoryCode? highlighted;
  final bool desktop;
  final int presentationRevision;

  Widget _conceal(Widget child, {String keyPrefix = 'inspector-spoiler'}) =>
      clip.secret
      ? SecretSpoiler(
          key: ValueKey('$keyPrefix-${clip.id}-$presentationRevision'),
          clipId: clip.id,
          controller: controller.spoilers,
          child: child,
        )
      : child;

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
                child: Align(
                  alignment: Alignment.topCenter,
                  child: _conceal(
                    _DetailImagePreview(clip: clip, controller: controller),
                  ),
                ),
              ),
              const Gap(AppSpacing.lg),
              Flexible(
                child: SingleChildScrollView(
                  primary: false,
                  key: const ValueKey<String>('history-detail-scroll-metadata'),
                  child: _conceal(
                    _HistoryMetadataTable(
                      clip: clip,
                      controller: controller,
                      revealConfidential: true,
                    ),
                    keyPrefix: 'inspector-metadata-spoiler',
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
            Align(
              alignment: Alignment.topLeft,
              child: _conceal(_sizedBody(context)),
            ),
            const Gap(AppSpacing.lg),
            _conceal(
              _HistoryMetadataTable(
                clip: clip,
                controller: controller,
                revealConfidential: true,
              ),
              keyPrefix: 'inspector-metadata-spoiler',
            ),
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
            : _sizedBody(context);
        return SingleChildScrollView(
          primary: false,
          key: const ValueKey<String>('history-detail-scroll-content'),
          padding: const EdgeInsets.symmetric(horizontal: AppSpacing.xs),
          child: Align(
            alignment: clip.contentKind == HistoryClipKind.image
                ? Alignment.topCenter
                : Alignment.topLeft,
            child: _conceal(content),
          ),
        );
      },
    );
  }

  Widget _sizedBody(BuildContext context) {
    final content = _body(context);
    if (!clip.secret ||
        body.length > 256 ||
        clip.contentKind == HistoryClipKind.image ||
        clip.contentKind == HistoryClipKind.color) {
      return content;
    }
    return IntrinsicWidth(child: content);
  }

  Widget _body(BuildContext context) {
    if (clip.contentKind == HistoryClipKind.color && clip.colorRgba != null) {
      return Row(
        children: [
          HistoryColorSwatch(rgba: clip.colorRgba!, size: 48),
          const Gap(AppSpacing.md),
          Expanded(
            child: HistorySelectableText(
              body,
              onCopy: (text) => controller.copyTextFrom(clip.id, text),
              style: Theme.of(context).typography.mono,
            ),
          ),
        ],
      );
    }
    if (highlighted != null) {
      return HistorySelectableText.rich(
        highlighted!.span,
        onCopy: (text) => controller.copyTextFrom(clip.id, text),
      );
    }
    if (clip.contentKind != HistoryClipKind.image) {
      return HistorySelectableText(
        body,
        onCopy: (text) => controller.copyTextFrom(clip.id, text),
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
        HistorySelectableText(
          body,
          onCopy: (text) => controller.copyTextFrom(clip.id, text),
        ),
      ],
    );
  }
}

class _HistoryMetadataTable extends StatelessWidget {
  const _HistoryMetadataTable({
    required this.clip,
    required this.controller,
    this.revealConfidential = false,
    this.compact = false,
  });

  final HistoryClip clip;
  final HistoryController controller;
  final bool compact;
  final bool revealConfidential;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final rows = _historyMetadataRows(
      context,
      clip,
      revealConfidential: revealConfidential,
    );
    final textStyle = AppTheme.inspectorTextStyle(context, compact: compact);
    final table = InspectorTable(
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
                : HistorySelectableText(
                    row.value,
                    onCopy: (text) => controller.copyTextFrom(clip.id, text),
                    style: textStyle.copyWith(
                      color: row.warning ? theme.colorScheme.destructive : null,
                    ),
                  ),
          ),
      ],
    );
    if (clip.contentKind != HistoryClipKind.file) return table;
    return ListenableBuilder(
      listenable: controller.downloadProgressChanges,
      child: table,
      builder: (context, child) {
        final operation = controller.fileOperations?.forClip(clip.id);
        final error = controller.downloadErrorFor(clip.id);
        final progress = controller.downloadProgressFor(clip.id);
        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            if (operation != null) ...[
              FileOperationStatus(operation: operation),
              const Gap(AppSpacing.md),
            ] else if (error != null) ...[
              StateView.error(title: 'Download failed', message: error),
              const Gap(AppSpacing.md),
            ] else if (controller.isDownloadPending(clip.id)) ...[
              Text(
                progress == null
                    ? 'Downloading…'
                    : 'Downloading ${(progress * 100).floor()}%',
              ),
              const Gap(AppSpacing.md),
            ],
            child!,
          ],
        );
      },
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
  HistoryClip clip, {
  bool revealConfidential = false,
}) {
  if (clip.secret && !revealConfidential) {
    return [(label: 'Privacy', value: 'Confidential', warning: false)];
  }
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
      (label: 'Size', value: formatBytes(sizeBytes), warning: false),
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
            return const SizedBox.square(
              dimension: AppIconSize.hero,
              child: Center(
                child: AppIconFrame(
                  size: AppIconSize.hero,
                  child: Icon(LucideIcons.image, size: AppIconSize.hero),
                ),
              ),
            );
          }
          if (preview.width <= 0 || preview.height <= 0) {
            return const AppIconFrame(
              size: AppIconSize.hero,
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
          return SizedBox(
            key: const ValueKey<String>('history-detail-image-viewport'),
            width: sourceWidth * scale,
            height: sourceHeight * scale,
            child: Image.memory(
              preview.bytes,
              key: ValueKey<String>('history-detail-image-${clip.id}'),
              fit: BoxFit.contain,
              alignment: Alignment.center,
              cacheWidth: math.min(
                preview.width,
                (sourceWidth * scale * MediaQuery.devicePixelRatioOf(context))
                    .ceil()
                    .clamp(1, preview.width),
              ),
              cacheHeight: math.min(
                preview.height,
                (sourceHeight * scale * MediaQuery.devicePixelRatioOf(context))
                    .ceil()
                    .clamp(1, preview.height),
              ),
              errorBuilder: (context, error, stackTrace) => const Center(
                child: AppIconFrame(
                  size: AppIconSize.hero,
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
