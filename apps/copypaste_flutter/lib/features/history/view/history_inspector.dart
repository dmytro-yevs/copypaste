import 'history_tags_dialog.dart';

import 'package:copypaste_flutter/app/theme/app_icon_frame.dart';

import '../../transfers/file_operation_tile.dart';
import '../../transfers/file_operations_view.dart';
import '../../../shared/format_bytes.dart';

import 'dart:async';

import 'history_selectable_text.dart';

import 'dart:math' as math;

import 'package:copypaste_flutter/app/theme/app_motion.dart';
import 'package:copypaste_flutter/app/theme/app_overlays.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/app/theme/app_toast.dart';
import 'package:copypaste_flutter/app/theme/app_tokens.dart';
import 'package:copypaste_flutter/features/devices/device_label.dart';
import 'package:copypaste_flutter/features/history/controller/history_controller.dart';
import 'package:copypaste_flutter/features/history/controller/history_module_actions.dart';
import 'package:copypaste_flutter/features/history/controller/history_transfer_controller.dart';
import 'package:copypaste_flutter/features/history/models/history_models.dart';
import 'package:copypaste_flutter/features/history/presentation/history_code_highlighter.dart';
import 'package:copypaste_flutter/features/history/presentation/history_clip_presentation.dart';
import 'package:copypaste_flutter/features/history/presentation/source_app_label.dart';
import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:copypaste_flutter/shared/inspector_table.dart';
import 'package:copypaste_flutter/shared/state_view.dart';
import 'package:copypaste_flutter/shared/system_date_time.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../presentation/secret_spoiler.dart';

import '../presentation/history_color_swatch.dart';
import '../presentation/history_expiry_text.dart';
import 'history_clip_menu.dart';
import 'history_delete_dialog.dart';
import 'history_module_action_runner.dart';
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
    final clip = controller.selection.selectedClip;
    if (clip?.id != _highlightClipId) {
      _highlightMemo.clear();
      _highlightClipId = clip?.id;
    }
    if (clip == null) {
      _highlightMemo.clear();
      return StateView.empty(
        title: context.l10n.historyInspectorEmptyTitle,
        message: context.l10n.historyInspectorEmptyMessage,
      );
    }
    final theme = Theme.of(context);
    final body = (clip.body ?? clip.preview).isEmpty
        ? context.l10n.historyNoPreview
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
                    tooltip: (context) => TooltipContainer(
                      child: Text(context.l10n.historyCloseClipDetails),
                    ),
                    child: Semantics(
                      label: context.l10n.historyCloseClipDetails,
                      button: true,
                      child: Button.ghost(
                        key: const ValueKey<String>(
                          'history-detail-inspector-close',
                        ),
                        style: AppTheme.actionButtonStyle(
                          const ButtonStyle.ghostIcon(),
                        ),
                        onPressed:
                            onClose ?? controller.selection.clearSelection,
                        child: const AppIconFrame(child: Icon(LucideIcons.x)),
                      ),
                    ),
                  ),
                ],
              ],
            ),
          ),
          Gap(inDrawer ? AppSpacing.sm : AppSpacing.lg),
          // The content keeps its natural height up to the free space; the
          // actions and metadata below always stay at the bottom.
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
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
              ],
            ),
          ),
          const Gap(AppSpacing.lg),
          // Metadata sits above the actions, and the actions close the card.
          if (!inDrawer) ...[
            Padding(
              padding: const EdgeInsets.fromLTRB(
                AppSpacing.xs,
                AppSpacing.zero,
                AppSpacing.xs,
                AppSpacing.xs,
              ),
              child: clip.masked
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
            const Gap(AppSpacing.md),
          ],
          Padding(
            padding: const EdgeInsets.fromLTRB(
              AppSpacing.xs,
              AppSpacing.zero,
              AppSpacing.xs,
              AppSpacing.xs,
            ),
            child: LayoutBuilder(
              builder: (context, constraints) =>
                  _footer(context, clip, constraints.maxWidth),
            ),
          ),
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
            child: Semantics(
              label: context.l10n.historyCloseClipDetails,
              button: true,
              child: Button.ghost(
                key: const ValueKey<String>('history-detail-drawer-close'),
                style: AppTheme.actionButtonStyle(
                  const ButtonStyle.ghostIcon(),
                ),
                onPressed: () => closeDrawer(context),
                child: const AppIconFrame(child: Icon(LucideIcons.x)),
              ),
            ),
          ),
          const Gap(AppSpacing.xs),
          Expanded(child: content),
        ],
      ),
    );
  }

  /// One row of actions: labelled buttons when they fit, icon buttons
  /// otherwise, and the least used buttons move into the More menu when even
  /// the icons do not fit.
  Widget _footer(BuildContext context, HistoryClip clip, double width) {
    final actions = historyClipActions(controller, clip);
    final moduleActions =
        controller.moduleActions?.actionsFor(clip) ?? const [];
    // Feature actions are buttons like the rest; only what does not fit
    // moves into the More menu.
    final slots = <Object>[
      _FooterSlot.copy,
      if (controller.transfers.canShare(clip)) _FooterSlot.share,
      if (clip.pinned && controller.tags.available) _FooterSlot.tags,
      if (controller.transfers.canDownloadSelected) _FooterSlot.download,
      _FooterSlot.pin,
      _FooterSlot.secret,
      ...actions,
      ...moduleActions,
      if (clip.contentKind == HistoryClipKind.image &&
          controller.ocr?.available == true)
        _FooterSlot.ocr,
      _FooterSlot.delete,
    ];
    final (:iconOnly, :hidden) = _footerLayout(
      context,
      clip,
      slots,
      overflowOrder: [
        ...moduleActions.reversed,
        ...actions.reversed,
        _FooterSlot.ocr,
        _FooterSlot.tags,
        _FooterSlot.secret,
        _FooterSlot.share,
        _FooterSlot.pin,
        _FooterSlot.delete,
        _FooterSlot.download,
      ],
      available: width,
    );
    final menuItems = <MenuItem>[
      for (final slot in slots)
        if (hidden.contains(slot) && slot != _FooterSlot.delete)
          if (slot case final HistoryClipAction action)
            ...historyClipActionMenuItems(
              context,
              controller: controller,
              clip: clip,
              actions: [action],
              keyPrefix: 'history-detail-action',
            )
          else if (slot case final HistoryModuleClipAction action)
            ...historyModuleActionMenuItems(
              context,
              controller: controller,
              clip: clip,
              actions: [action],
              keyPrefix: 'history-detail-action',
            )
          else
            _slotMenuItem(context, clip, slot as _FooterSlot),
      if (hidden.contains(_FooterSlot.delete)) ...[
        const MenuDivider(),
        _slotMenuItem(context, clip, _FooterSlot.delete),
      ],
    ];
    return Row(
      key: const ValueKey<String>('history-detail-actions'),
      spacing: AppSpacing.sm,
      children: [
        for (final slot in slots)
          if (!hidden.contains(slot) && slot != _FooterSlot.delete)
            if (slot case final HistoryClipAction action)
              _iconAction(context, clip, action)
            else if (slot case final HistoryModuleClipAction action)
              _moduleAction(context, clip, action)
            else
              _slotButton(
                context,
                clip,
                slot as _FooterSlot,
                iconOnly: iconOnly,
              ),
        if (menuItems.isNotEmpty) _moreActions(context, menuItems),
        if (!hidden.contains(_FooterSlot.delete))
          _slotButton(context, clip, _FooterSlot.delete, iconOnly: iconOnly),
      ],
    );
  }

  /// [overflowOrder] lists the buttons least used first: the order in which
  /// they move into the More menu.
  ({bool iconOnly, Set<Object> hidden}) _footerLayout(
    BuildContext context,
    HistoryClip clip,
    List<Object> slots, {
    required List<Object> overflowOrder,
    required double available,
  }) {
    final strings = context.l10n;
    final textual = clip.contentKind.isTextual;
    final icon = _buttonWidth(context, const ButtonStyle.outlineIcon(), null);
    double widest(ButtonStyle style, List<String> labels) => labels
        .map((label) => _buttonWidth(context, style, label))
        .reduce(math.max);
    double labelled(Object slot) => switch (slot) {
      _FooterSlot.copy =>
        _buttonWidth(context, const ButtonStyle.primary(), strings.commonCopy) +
            (textual ? icon : 0),
      _FooterSlot.tags => widest(const ButtonStyle.outline(), [
        strings.historyTags,
      ]),
      _FooterSlot.download => widest(const ButtonStyle.secondary(), [
        strings.historyDownload,
        strings.historyShowFile,
      ]),
      _FooterSlot.pin => widest(const ButtonStyle.secondary(), [
        strings.commonPin,
        strings.historyPinnedButton,
      ]),
      _FooterSlot.delete => _buttonWidth(
        context,
        const ButtonStyle.destructive(),
        strings.commonDelete,
      ),
      _ => icon,
    };
    double iconic(Object slot) =>
        slot == _FooterSlot.copy && textual ? icon * 2 : icon;
    bool fits(
      Iterable<Object> shown,
      double Function(Object) widthOf, {
      required bool more,
    }) {
      final widths = [...shown.map(widthOf), if (more) icon];
      final required =
          widths.fold<double>(0, (sum, width) => sum + width) +
          (widths.length - 1) * AppSpacing.sm;
      return required <= available;
    }

    if (fits(slots, labelled, more: false)) {
      return (iconOnly: false, hidden: const {});
    }
    if (fits(slots, iconic, more: false)) {
      return (iconOnly: true, hidden: const {});
    }
    final hidden = <Object>{};
    for (final slot in overflowOrder) {
      if (!slots.contains(slot)) continue;
      hidden.add(slot);
      final shown = slots.where((slot) => !hidden.contains(slot));
      if (fits(shown, iconic, more: true)) break;
    }
    return (iconOnly: true, hidden: hidden);
  }

  double _buttonWidth(
    BuildContext context,
    ButtonStyle buttonStyle,
    String? label,
  ) {
    final theme = Theme.of(context);
    final style = AppTheme.actionButtonStyle(buttonStyle);
    final iconWidth = style.iconTheme(context, const {}).size ?? AppIconSize.sm;
    final padding = style
        .padding(context, const {})
        .resolve(Directionality.of(context));
    if (label == null) {
      return math.max(
        padding.horizontal + iconWidth,
        AppTheme.controlHeight(context),
      );
    }
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

  Widget _slotButton(
    BuildContext context,
    HistoryClip clip,
    _FooterSlot slot, {
    required bool iconOnly,
  }) {
    final strings = context.l10n;
    Widget labelled({
      required Key key,
      required String label,
      required Widget icon,
      required ButtonStyle labelStyle,
      required ButtonStyle iconStyle,
      required VoidCallback? onPressed,
      bool? toggled,
      bool iconOnlyAlways = false,
    }) {
      final asIcon = iconOnly || iconOnlyAlways;
      return Tooltip(
        tooltip: (_) => TooltipContainer(child: Text(label)),
        child: Semantics(
          label: asIcon ? label : null,
          button: true,
          toggled: toggled,
          child: Button(
            key: key,
            style: AppTheme.actionButtonStyle(asIcon ? iconStyle : labelStyle),
            onPressed: onPressed,
            leading: asIcon ? null : icon,
            child: asIcon ? icon : Text(label),
          ),
        ),
      );
    }

    switch (slot) {
      case _FooterSlot.copy:
        return _copyGroup(context, clip, iconOnly: iconOnly);
      case _FooterSlot.share:
        return Builder(
          builder: (buttonContext) => labelled(
            key: const ValueKey('history-detail-share'),
            label: strings.historyShare,
            icon: const AppIconFrame(child: Icon(LucideIcons.share)),
            labelStyle: const ButtonStyle.outlineIcon(),
            iconStyle: const ButtonStyle.outlineIcon(),
            iconOnlyAlways: true,
            onPressed: () => unawaited(
              shareHistoryClip(
                buttonContext,
                controller: controller,
                clip: clip,
              ),
            ),
          ),
        );
      case _FooterSlot.tags:
        return labelled(
          key: const ValueKey('history-inspector-tags'),
          label: strings.historyTags,
          icon: const Icon(LucideIcons.tags, size: AppIconSize.sm),
          labelStyle: const ButtonStyle.outline(),
          iconStyle: const ButtonStyle.outlineIcon(),
          onPressed: () => showHistoryTagsDialog(
            context,
            controller: controller,
            clipIds: [clip.id],
          ),
        );
      case _FooterSlot.download:
        return _DownloadButton(
          controller: controller,
          clip: clip,
          iconOnly: iconOnly,
          onDownload: () => _download(context),
        );
      case _FooterSlot.pin:
        final variance = clip.pinned
            ? ButtonVariance.secondary
            : ButtonVariance.outline;
        return labelled(
          key: ValueKey<String>('history-pin-${clip.id}'),
          label: clip.pinned ? strings.historyPinnedButton : strings.commonPin,
          icon: AppIconFrame(
            child: Icon(clip.pinned ? LucideIcons.pinOff : LucideIcons.pin),
          ),
          labelStyle: ButtonStyle(variance: variance),
          iconStyle: ButtonStyle(
            variance: variance,
            density: ButtonDensity.icon,
          ),
          toggled: clip.pinned,
          onPressed: controller.mutations.isPinPending(clip.id)
              ? null
              : () => controller.mutations.togglePin(clip),
        );
      case _FooterSlot.secret:
        return labelled(
          key: const ValueKey('history-detail-secret'),
          label: clip.secret
              ? strings.historyRemoveSpoiler
              : strings.historyAddSpoiler,
          icon: AppIconFrame(
            child: Icon(
              clip.secret ? LucideIcons.lockOpen : LucideIcons.lockKeyhole,
            ),
          ),
          labelStyle: const ButtonStyle.outlineIcon(),
          iconStyle: const ButtonStyle.outlineIcon(),
          iconOnlyAlways: true,
          toggled: clip.secret,
          onPressed: controller.mutations.isSecretPending(clip.id)
              ? null
              : () => unawaited(_toggleSecret(context, clip)),
        );
      case _FooterSlot.ocr:
        final ocr = controller.ocr!;
        return Tooltip(
          tooltip: (_) =>
              TooltipContainer(child: Text(strings.historyRecognizeImageText)),
          child: Semantics(
            label: strings.historyRecognizeImageText,
            button: true,
            child: Button.secondary(
              key: const ValueKey<String>('history-ocr'),
              style: AppTheme.actionButtonStyle(
                const ButtonStyle.secondaryIcon(),
              ),
              onPressed: ocr.canRun
                  ? () => showHistoryOcrDialog(
                      context,
                      controller: ocr,
                      clip: clip,
                    )
                  : null,
              child: const AppIconFrame(child: Icon(LucideIcons.scanText)),
            ),
          ),
        );
      case _FooterSlot.delete:
        return labelled(
          key: const ValueKey<String>('history-detail-delete'),
          label: strings.commonDelete,
          icon: const AppIconFrame(child: Icon(LucideIcons.trash2)),
          labelStyle: const ButtonStyle.destructive(),
          iconStyle: const ButtonStyle.destructiveIcon(),
          onPressed: () => showHistoryDeleteDialog(
            context,
            controller: controller,
            clipId: clip.id,
          ),
        );
    }
  }

  Widget _copyGroup(
    BuildContext context,
    HistoryClip clip, {
    required bool iconOnly,
  }) => ButtonGroup(
    children: [
      Tooltip(
        tooltip: (_) => TooltipContainer(child: Text(context.l10n.commonCopy)),
        child: Semantics(
          label: iconOnly ? context.l10n.commonCopy : null,
          button: true,
          child: Button.primary(
            key: const ValueKey<String>('history-detail-copy'),
            style: AppTheme.actionButtonStyle(
              iconOnly
                  ? const ButtonStyle.primaryIcon()
                  : const ButtonStyle.primary(),
            ),
            onPressed: () => _copy(context, plainText: false),
            leading: iconOnly
                ? null
                : const AppIconFrame(child: Icon(LucideIcons.copy)),
            child: iconOnly
                ? const AppIconFrame(child: Icon(LucideIcons.copy))
                : Text(context.l10n.commonCopy),
          ),
        ),
      ),
      if (clip.contentKind.isTextual)
        Semantics(
          label: context.l10n.historyCopyOptions,
          button: true,
          child: ComponentTheme<SelectTheme>(
            data: AppTheme.selectTheme(
              context,
              variant: AppSelectVariant.primaryAction,
            ),
            child: Select<bool>(
              key: const ValueKey<String>('history-copy-options'),
              value: false,
              expandIcon: null,
              itemBuilder: (context, _) => AppIconFrame(
                child: Icon(
                  LucideIcons.chevronDown,
                  color: Theme.of(context).colorScheme.primaryForeground,
                ),
              ),
              onChanged: (plainText) {
                if (plainText == true) {
                  unawaited(_copy(context, plainText: true));
                }
              },
              popup: SelectPopup<bool>.noVirtualization(
                items: SelectItemList(
                  children: [
                    SelectItemButton<bool>(
                      value: true,
                      child: Row(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          const AppIconFrame(
                            size: AppIconSize.sm,
                            child: Icon(
                              LucideIcons.alignLeft,
                              size: AppIconSize.sm,
                            ),
                          ),
                          const Gap(AppSpacing.sm),
                          Text(context.l10n.historyCopyPlainText),
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
  );

  /// The More menu entry for a footer button that did not fit.
  MenuItem _slotMenuItem(
    BuildContext context,
    HistoryClip clip,
    _FooterSlot slot,
  ) {
    final strings = context.l10n;
    MenuItem item(
      IconData icon,
      String label,
      VoidCallback? onPressed, {
      Color? color,
    }) => MenuButton(
      key: ValueKey<String>('history-detail-menu-${slot.name}'),
      enabled: onPressed != null,
      onPressed: onPressed == null ? null : (_) => onPressed(),
      leading: AppIconFrame(
        size: AppIconSize.sm,
        child: Icon(icon, size: AppIconSize.sm, color: color),
      ),
      child: Text(label, style: TextStyle(color: color)),
    );

    switch (slot) {
      case _FooterSlot.copy:
        return item(
          LucideIcons.copy,
          strings.commonCopy,
          () => unawaited(_copy(context, plainText: false)),
        );
      case _FooterSlot.share:
        return item(
          LucideIcons.share,
          strings.historyShare,
          () => unawaited(
            shareHistoryClip(context, controller: controller, clip: clip),
          ),
        );
      case _FooterSlot.tags:
        return item(
          LucideIcons.tags,
          strings.historyTagsAction,
          () => unawaited(
            showHistoryTagsDialog(
              context,
              controller: controller,
              clipIds: [clip.id],
            ),
          ),
        );
      case _FooterSlot.download:
        final transfers = controller.transfers;
        final operation = transfers.fileOperations?.forClip(clip.id);
        if (transfers.isDownloadPending(clip.id)) {
          return item(
            LucideIcons.x,
            strings.commonCancel,
            transfers.isDownloadCancelling(clip.id)
                ? null
                : () => transfers.cancelDownload(clip.id),
          );
        }
        if (operation?.phase == 'completed' && operation?.localPath != null) {
          return item(
            LucideIcons.folderOpen,
            (clip.file?.isSet ?? false)
                ? strings.fileSetShowFolder
                : strings.historyShowFile,
            () => transfers.fileOperations!.openFile(operation!),
          );
        }
        return item(
          LucideIcons.download,
          strings.historyDownload,
          () => unawaited(_download(context)),
        );
      case _FooterSlot.pin:
        return item(
          clip.pinned ? LucideIcons.pinOff : LucideIcons.pin,
          clip.pinned ? strings.historyUnpinClip : strings.commonPin,
          controller.mutations.isPinPending(clip.id)
              ? null
              : () => controller.mutations.togglePin(clip),
        );
      case _FooterSlot.secret:
        return item(
          clip.secret ? LucideIcons.lockOpen : LucideIcons.lockKeyhole,
          clip.secret
              ? strings.historyRemoveSpoiler
              : strings.historyAddSpoiler,
          controller.mutations.isSecretPending(clip.id)
              ? null
              : () => unawaited(_toggleSecret(context, clip)),
        );
      case _FooterSlot.ocr:
        final ocr = controller.ocr!;
        return item(
          LucideIcons.scanText,
          strings.historyRecognizeImageText,
          ocr.canRun
              ? () => unawaited(
                  showHistoryOcrDialog(context, controller: ocr, clip: clip),
                )
              : null,
        );
      case _FooterSlot.delete:
        return item(
          LucideIcons.trash2,
          strings.commonDelete,
          () => unawaited(
            showHistoryDeleteDialog(
              context,
              controller: controller,
              clipId: clip.id,
            ),
          ),
          color: Theme.of(context).colorScheme.destructive,
        );
    }
  }

  Future<void> _toggleSecret(BuildContext context, HistoryClip clip) async {
    final changed = await controller.mutations.toggleSecret(clip);
    if (changed || !context.mounted) return;
    AppToast.show(
      context,
      title: context.l10n.historyUpdateFailedTitle,
      message: context.l10n.historyErrorSecrecyUpdate,
      tone: AppToastTone.error,
    );
  }

  Widget _iconAction(
    BuildContext context,
    HistoryClip clip,
    HistoryClipAction action,
  ) {
    final label = action.label(context, clip);
    return Tooltip(
      tooltip: (_) => TooltipContainer(child: Text(label)),
      child: Semantics(
        label: label,
        button: true,
        child: Button.outline(
          key: ValueKey<String>('history-detail-action-${action.id}'),
          style: AppTheme.actionButtonStyle(const ButtonStyle.outlineIcon()),
          onPressed: () => unawaited(
            runHistoryClipAction(
              context,
              controller: controller,
              clip: clip,
              action: action,
            ),
          ),
          child: AppIconFrame(child: Icon(action.icon(clip))),
        ),
      ),
    );
  }

  Widget _moduleAction(
    BuildContext context,
    HistoryClip clip,
    HistoryModuleClipAction action,
  ) {
    final label = action.action.title;
    return Tooltip(
      tooltip: (_) => TooltipContainer(child: Text(label)),
      child: Semantics(
        label: label,
        button: true,
        child: Button.outline(
          key: ValueKey<String>('history-detail-action-${action.id}'),
          style: AppTheme.actionButtonStyle(const ButtonStyle.outlineIcon()),
          onPressed: () => unawaited(
            runHistoryModuleAction(
              context,
              controller: controller,
              clip: clip,
              action: action,
            ),
          ),
          child: AppIconFrame(child: Icon(historyModuleActionIcon(action))),
        ),
      ),
    );
  }

  Widget _moreActions(BuildContext context, List<MenuItem> items) => Tooltip(
    tooltip: (_) =>
        TooltipContainer(child: Text(context.l10n.historyMoreActions)),
    child: Semantics(
      label: context.l10n.historyMoreActions,
      button: true,
      child: Builder(
        builder: (anchor) => Button.outline(
          key: const ValueKey<String>('history-detail-more'),
          style: AppTheme.actionButtonStyle(const ButtonStyle.outlineIcon()),
          onPressed: () => unawaited(
            showDropdown<void>(
              context: anchor,
              alignment: AlignmentDirectional.bottomEnd,
              anchorAlignment: AlignmentDirectional.topEnd,
              builder: (popupContext) => AppTheme.popupSurface(
                popupContext,
                child: DropdownMenu(children: items),
              ),
            ).future,
          ),
          child: const AppIconFrame(child: Icon(LucideIcons.ellipsis)),
        ),
      ),
    ),
  );

  Future<void> _copy(BuildContext context, {required bool plainText}) async {
    final copied = await controller.transfers.copySelected(
      plainText: plainText,
    );
    if (!copied || !context.mounted) return;
    AppToast.show(
      context,
      title: context.l10n.historyCopiedTitle,
      message: plainText
          ? context.l10n.historyPlainTextCopied
          : context.l10n.historyClipCopied,
      tone: AppToastTone.success,
    );
  }

  Future<void> _download(BuildContext context) async {
    final set = controller.selection.selectedClip?.file?.isSet ?? false;
    final result = await controller.transfers.downloadSelected();
    if (result != HistoryFileDownloadResult.saved || !context.mounted) return;
    AppToast.show(
      context,
      title: context.l10n.historyDownloadedTitle,
      message: set
          ? context.l10n.historyFilesSaved
          : context.l10n.historyFileSaved,
      tone: AppToastTone.success,
    );
  }
}

/// The inspector footer's buttons in display order; More sits before Delete.
enum _FooterSlot { copy, share, tags, download, pin, secret, ocr, delete }

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
    listenable: widget.controller.transfers.downloadProgressChanges,
    builder: (context, _) => _buildButton(context),
  );

  Widget _buildButton(BuildContext context) {
    final controller = widget.controller;
    final id = widget.clip.id;
    final pending = controller.transfers.isDownloadPending(id);
    final failed = controller.transfers.downloadFailedFor(id);
    final cancelling = controller.transfers.isDownloadCancelling(id);
    final progress = controller.transfers.downloadProgressFor(id);
    final operation = controller.transfers.fileOperations?.forClip(id);
    final saved =
        operation?.phase == 'completed' && operation?.localPath != null;
    final percentage = ((progress ?? 0) * 100).floor();
    final showCancel = pending && _hovered && !cancelling;
    final strings = context.l10n;
    final label = cancelling
        ? strings.historyDownloadCancelling
        : showCancel
        ? strings.commonCancel
        : pending && progress != null
        ? strings.historyDownloadProgressPercent(percentage)
        : pending
        ? strings.historyDownloading
        : saved
        ? (widget.clip.file?.isSet ?? false)
              ? strings.fileSetShowFolder
              : strings.historyShowFile
        : failed
        ? strings.commonRetry
        : strings.historyDownload;
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
                ? strings.historyDownloadCancellingTooltip
                : pending
                ? strings.historyDownloadingCancelTooltip(percentage)
                : failed
                ? controller.transfers.downloadErrorFor(id) ??
                      strings.historyDownloadFailedRetryTooltip
                : strings.historyDownload,
          ),
        ),
        child: Semantics(
          label: widget.iconOnly
              ? pending
                    ? strings.historyCancelDownloadSemantics(percentage)
                    : failed
                    ? strings.historyRetryDownload
                    : strings.historyDownload
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
                  ? () => controller.transfers.cancelDownload(id)
                  : saved
                  ? () => controller.transfers.fileOperations!.openFile(
                      operation!,
                    )
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
                                  child: FittedBox(
                                    child: Text(
                                      strings.historyDownloadProgressPercent(
                                        percentage,
                                      ),
                                    ),
                                  ),
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
      clip.masked
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
    if (!clip.masked ||
        body.length > 256 ||
        _hasFileMembers ||
        clip.contentKind == HistoryClipKind.image ||
        clip.contentKind == HistoryClipKind.color) {
      return content;
    }
    return IntrinsicWidth(child: content);
  }

  bool get _hasFileMembers => clip.file?.listsMembers ?? false;

  Widget _body(BuildContext context) {
    if (_hasFileMembers) {
      return _FileMemberList(clip: clip, controller: controller);
    }
    if (clip.contentKind == HistoryClipKind.color && clip.colorRgba != null) {
      return Row(
        children: [
          HistoryColorSwatch(rgba: clip.colorRgba!, size: 48),
          const Gap(AppSpacing.md),
          Expanded(
            child: HistorySelectableText(
              body,
              onCopy: (text) =>
                  controller.transfers.copyTextFrom(clip.id, text),
              style: Theme.of(context).typography.mono,
            ),
          ),
        ],
      );
    }
    if (highlighted != null) {
      return HistorySelectableText.rich(
        highlighted!.span,
        onCopy: (text) => controller.transfers.copyTextFrom(clip.id, text),
      );
    }
    if (clip.contentKind != HistoryClipKind.image) {
      return HistorySelectableText(
        body,
        onCopy: (text) => controller.transfers.copyTextFrom(clip.id, text),
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
          onCopy: (text) => controller.transfers.copyTextFrom(clip.id, text),
        ),
      ],
    );
  }
}

class _FileMemberList extends StatelessWidget {
  const _FileMemberList({required this.clip, required this.controller});

  final HistoryClip clip;
  final HistoryController controller;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final detailStyle = theme.typography.xSmall.copyWith(
      color: theme.colorScheme.mutedForeground,
    );
    final members = clip.file!.files;
    return Column(
      key: const ValueKey<String>('history-detail-file-members'),
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        for (final (index, member) in members.indexed) ...[
          if (index > 0) const Gap(AppSpacing.sm),
          Row(
            children: [
              Expanded(
                child: Text(
                  member.name,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              const Gap(AppSpacing.sm),
              Text(formatBytes(member.sizeBytes), style: detailStyle),
            ],
          ),
          if (member.sourceReference?.trim().isNotEmpty == true)
            HistorySelectableText(
              member.sourceReference!,
              onCopy: (text) =>
                  controller.transfers.copyTextFrom(clip.id, text),
              style: detailStyle,
            ),
        ],
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
            value:
                _historyMetadataValue(
                  context,
                  row: row,
                  clip: clip,
                  controller: controller,
                  style: textStyle,
                ) ??
                HistorySelectableText(
                  row.value,
                  onCopy: (text) =>
                      controller.transfers.copyTextFrom(clip.id, text),
                  style: textStyle.copyWith(
                    color: row.warning ? theme.colorScheme.destructive : null,
                  ),
                ),
          ),
      ],
    );
    if (clip.contentKind != HistoryClipKind.file) return table;
    return ListenableBuilder(
      listenable: controller.transfers.downloadProgressChanges,
      child: table,
      builder: (context, child) {
        final operation = controller.transfers.fileOperations?.forClip(clip.id);
        final error = controller.transfers.downloadErrorFor(clip.id);
        final progress = controller.transfers.downloadProgressFor(clip.id);
        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            if (operation != null) ...[
              FileOperationStatus(operation: operation),
              const Gap(AppSpacing.md),
            ] else if (error != null) ...[
              FileOperationFailure(message: error),
              const Gap(AppSpacing.md),
            ] else if (controller.transfers.isDownloadPending(clip.id)) ...[
              Text(
                progress == null
                    ? context.l10n.historyDownloading
                    : context.l10n.historyDownloadingPercent(
                        (progress * 100).floor(),
                      ),
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

/// Rows whose value is more than selectable text; null for plain rows.
Widget? _historyMetadataValue(
  BuildContext context, {
  required _HistoryMetadataRow row,
  required HistoryClip clip,
  required HistoryController controller,
  required TextStyle style,
}) {
  final strings = context.l10n;
  if (row.label == strings.historyMetadataObservedApp) {
    final provenance = HistoryClipPresentation.provenance(
      context.l10n,
      clip.annotations.provenance,
    );
    final source = row.value.isEmpty
        ? null
        : SourceAppLabel(
            key: const ValueKey<String>('history-detail-source-app'),
            name: row.value,
            icon: controller.media.requestSourceIcon(clip.sourceAppIconId),
            style: style,
          );
    if (provenance == null) return source;
    return Wrap(
      spacing: AppSpacing.sm,
      runSpacing: AppSpacing.xs,
      crossAxisAlignment: WrapCrossAlignment.center,
      children: [
        ?source,
        SecondaryBadge(
          key: const ValueKey<String>('history-detail-provenance'),
          child: Text(provenance),
        ),
      ],
    );
  }
  if (row.label == strings.historyMetadataDevice) {
    return DeviceLabel(
      key: const ValueKey<String>('history-detail-device'),
      name: row.value,
      deviceClass: clip.originDeviceClass,
      style: style,
    );
  }
  if (row.label == strings.historyMetadataExpiry) {
    return HistoryExpiryText(
      expiresAt: clip.annotations.expiresAt!,
      style: style.copyWith(color: Theme.of(context).colorScheme.destructive),
    );
  }
  if (row.label == strings.historyMetadataRuleTags) {
    return Wrap(
      key: const ValueKey<String>('history-detail-rule-tags'),
      spacing: AppSpacing.xs,
      runSpacing: AppSpacing.xs,
      children: [
        for (final name in controller.tags.names(clip.annotations.ruleTagIds))
          SecondaryBadge(child: Text(strings.historyRuleTag(name))),
      ],
    );
  }
  return null;
}

List<_HistoryMetadataRow> _historyMetadataRows(
  BuildContext context,
  HistoryClip clip, {
  bool revealConfidential = false,
}) {
  final strings = context.l10n;
  if (clip.masked && !revealConfidential) {
    return [
      (
        label: strings.historyMetadataPrivacy,
        value: strings.historyMetadataConfidential,
        warning: false,
      ),
    ];
  }
  final type = clip.file?.mimeType ?? clip.contentType;
  final sizeBytes = clip.image?.sizeBytes ?? clip.file?.sizeBytes;
  final annotations = clip.annotations;
  return [
    (
      label: strings.historyMetadataCaptured,
      value: formatSystemDateTime(context, clip.createdAt),
      warning: false,
    ),
    if (annotations.expiresAt != null && !annotations.keep)
      (label: strings.historyMetadataExpiry, value: '', warning: true),
    if (clip.sourceApp != null || annotations.provenance != null)
      (
        label: strings.historyMetadataObservedApp,
        value: clip.sourceApp ?? '',
        warning: false,
      ),
    if (clip.origin != null)
      (
        label: strings.historyMetadataDevice,
        value: clip.origin!,
        warning: false,
      ),
    (
      label: strings.historyMetadataClipType,
      value: clip.contentKind.label,
      warning: false,
    ),
    (label: strings.historyMetadataType, value: type, warning: false),
    if (clip.file?.sourceReference != null && !clip.file!.listsMembers)
      (
        label: clip.file!.sourceReference!.startsWith('content://')
            ? 'URI'
            : strings.historyMetadataPath,
        value: clip.file!.sourceReference!,
        warning: false,
      ),
    if (clip.file?.name != null && !clip.file!.listsMembers)
      (
        label: strings.historyMetadataFile,
        value: clip.file!.name!,
        warning: false,
      ),
    if (clip.image != null)
      (
        label: strings.historyMetadataResolution,
        value: '${clip.image!.width} × ${clip.image!.height}',
        warning: false,
      ),
    if (sizeBytes != null)
      (
        label: strings.historyMetadataSize,
        value: formatBytes(sizeBytes),
        warning: false,
      ),
    if (clip.file?.isSet ?? false)
      (
        label: strings.historyMetadataFiles,
        value: '${clip.file!.fileCount}',
        warning: false,
      ),
    if (annotations.ruleTagIds.isNotEmpty)
      (label: strings.historyMetadataRuleTags, value: '', warning: false),
    if (clip.tooLargeToSync)
      (
        label: strings.historyMetadataSync,
        value: strings.historyMetadataTooLargeToSync,
        warning: true,
      ),
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
        future: controller.media.requestImagePreview(clip.id, maxEdge: 1024),
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
          return Button(
            key: const ValueKey<String>('history-detail-image-open'),
            style: AppTheme.mediaButtonStyle,
            onPressed: () => unawaited(_openPreview(context)),
            child: SizedBox(
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
                  (sourceHeight *
                          scale *
                          MediaQuery.devicePixelRatioOf(context))
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
            ),
          );
        },
      ),
    );
  }

  Future<void> _openPreview(BuildContext context) {
    final window = MediaQuery.sizeOf(context);
    // Decoded only as large as the preview can show it.
    final maxEdge =
        (math.max(window.width, window.height) *
                AppOverlaySize.previewFactor *
                MediaQuery.devicePixelRatioOf(context))
            .ceil();
    final image = controller.media.requestImagePreview(
      clip.id,
      maxEdge: maxEdge,
    );
    return AppOverlays.showPreview(
      context,
      builder: (context, bounds) => FutureBuilder<HistoryImagePreview?>(
        future: image,
        builder: (context, snapshot) {
          final preview = snapshot.data;
          if (preview == null || preview.width <= 0 || preview.height <= 0) {
            return const SizedBox.shrink();
          }
          return ConstrainedBox(
            constraints: BoxConstraints.loose(bounds),
            child: Image.memory(
              preview.bytes,
              key: ValueKey<String>('history-preview-image-${clip.id}'),
              fit: BoxFit.contain,
              filterQuality: FilterQuality.medium,
            ),
          );
        },
      ),
    );
  }
}
