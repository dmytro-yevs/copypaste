import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_toast.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../../../platform/desktop/quick_paste_host.dart';
import '../../../shared/adaptive_breakpoints.dart';
import '../../../shared/format_bytes.dart';
import '../../../shared/state_view.dart';
import '../controller/transform_controller.dart';
import '../controller/transform_entries.dart';
import '../models/transform_models.dart';
import '../presentation/transform_messages.dart';
import '../repository/transform_recents_store.dart';
import '../repository/transform_repository.dart';
import 'transform_param_fields.dart';

/// Opens the transform palette for [source]. The palette owns its session:
/// the catalogue and any running preview are released when it closes.
///
/// [pasteForeground] enables Paste; it must bring the target application
/// forward before it pastes. Without it the primary action is Copy.
Future<TransformActionOutcome?> showTransformPalette(
  BuildContext context, {
  required TransformRepository repository,
  required TransformSource source,
  TransformRecentsStore? recents,
  Future<PasteOutcome> Function()? pasteForeground,
}) async {
  final controller = TransformController(
    repository: repository,
    source: source,
    recents: recents,
    pasteForeground: pasteForeground,
  );
  unawaited(controller.open());
  try {
    final outcome = await showOverlay<TransformActionOutcome>(
      context,
      DialogConfiguration(
        alignment: defaultTargetPlatform == TargetPlatform.android
            ? Alignment.bottomCenter
            : Alignment.center,
      ),
      builder: (dialogContext) => SizedBox(
        width: math.min(
          AppLayoutSize.transformPaletteWidth,
          MediaQuery.sizeOf(dialogContext).width - AppSpacing.lg * 2,
        ),
        child: TransformPalette(
          controller: controller,
          onDone: (outcome) => Navigator.of(dialogContext).pop(outcome),
        ),
      ),
    ).future;
    if (outcome != null && context.mounted) {
      _announceOutcome(context, outcome);
    }
    return outcome;
  } finally {
    controller.dispose();
  }
}

void _announceOutcome(BuildContext context, TransformActionOutcome outcome) {
  final l10n = context.l10n;
  final (String, String)? message = switch (outcome) {
    TransformActionOutcome(status: TransformActionStatus.onClipboard) => (
      l10n.transformsPasteFailedTitle,
      l10n.transformsPasteFailedMessage,
    ),
    TransformActionOutcome(action: TransformRunAction.copy) => (
      l10n.historyCopiedTitle,
      l10n.transformsCopiedMessage,
    ),
    TransformActionOutcome(action: TransformRunAction.saveNewClip) => (
      l10n.transformsSavedTitle,
      l10n.transformsSavedMessage,
    ),
    _ => null,
  };
  if (message == null) return;
  AppToast.show(
    context,
    title: message.$1,
    message: message.$2,
    tone: outcome.status == TransformActionStatus.onClipboard
        ? AppToastTone.information
        : AppToastTone.success,
  );
}

/// The palette: a filtered list of transforms and pipelines, the chain
/// being built, its parameters, a live preview and the output actions.
class TransformPalette extends StatefulWidget {
  const TransformPalette({
    super.key,
    required this.controller,
    required this.onDone,
  });

  final TransformController controller;

  /// Closes the palette; null when nothing ran.
  final void Function(TransformActionOutcome? outcome) onDone;

  @override
  State<TransformPalette> createState() => _TransformPaletteState();
}

enum _PreviewPane { output, input, changes }

class _TransformPaletteState extends State<TransformPalette> {
  final _listScope = FocusScopeNode(debugLabel: 'transform-palette-list');
  final _paramsScope = FocusScopeNode(debugLabel: 'transform-palette-params');
  final _highlight = _HighlightTracker();
  _PreviewPane _pane = _PreviewPane.output;

  TransformController get _controller => widget.controller;

  bool get _desktop =>
      defaultTargetPlatform != TargetPlatform.android &&
      defaultTargetPlatform != TargetPlatform.iOS;

  @override
  void dispose() {
    _listScope.dispose();
    _paramsScope.dispose();
    super.dispose();
  }

  Future<void> _act(Future<TransformActionOutcome?> Function() action) async {
    final outcome = await action();
    if (!mounted || outcome == null) return;
    if (outcome.status != TransformActionStatus.failed) widget.onDone(outcome);
  }

  bool get _modifier {
    final keyboard = HardwareKeyboard.instance;
    return defaultTargetPlatform == TargetPlatform.macOS
        ? keyboard.isMetaPressed
        : keyboard.isControlPressed;
  }

  static const _digits = [
    LogicalKeyboardKey.digit1,
    LogicalKeyboardKey.digit2,
    LogicalKeyboardKey.digit3,
    LogicalKeyboardKey.digit4,
    LogicalKeyboardKey.digit5,
    LogicalKeyboardKey.digit6,
    LogicalKeyboardKey.digit7,
    LogicalKeyboardKey.digit8,
    LogicalKeyboardKey.digit9,
  ];

  /// Palette shortcuts. A parameter field keeps its own text editing keys,
  /// so only Escape and Mod+, reach the palette from there.
  KeyEventResult _onKey(FocusNode node, KeyEvent event) {
    if (event is! KeyDownEvent && event is! KeyRepeatEvent) {
      return KeyEventResult.ignored;
    }
    final key = event.logicalKey;
    final keyboard = HardwareKeyboard.instance;
    final mod = _modifier;
    final shift = keyboard.isShiftPressed;
    if (key == LogicalKeyboardKey.escape) {
      _back();
      return KeyEventResult.handled;
    }
    if (mod && key == LogicalKeyboardKey.comma) {
      _focusFirst(_paramsScope);
      return KeyEventResult.handled;
    }
    if (_paramsScope.hasFocus || event is KeyRepeatEvent) {
      return KeyEventResult.ignored;
    }
    void run(Future<TransformActionOutcome?> Function() action) =>
        unawaited(_act(action));
    final selected = _controller.selectedStep;
    if (keyboard.isAltPressed &&
        selected != null &&
        selected < _controller.committedCount &&
        (key == LogicalKeyboardKey.arrowUp ||
            key == LogicalKeyboardKey.arrowDown)) {
      _controller.moveStep(
        selected,
        key == LogicalKeyboardKey.arrowUp ? -1 : 1,
      );
      return KeyEventResult.handled;
    }
    if (!mod) return KeyEventResult.ignored;
    if (shift && key == LogicalKeyboardKey.enter) {
      run(_controller.pasteOriginal);
    } else if (!shift && key == LogicalKeyboardKey.keyC) {
      run(_controller.copy);
    } else if (shift && key == LogicalKeyboardKey.keyS) {
      unawaited(_savePipeline());
    } else if (key == LogicalKeyboardKey.keyS) {
      run(_controller.saveNewClip);
    } else if (key == LogicalKeyboardKey.keyR) {
      run(_controller.repeatLast);
    } else if (_digits.indexOf(key) case final index when index >= 0) {
      run(() => _controller.runFavorite(index));
    } else {
      return KeyEventResult.ignored;
    }
    return KeyEventResult.handled;
  }

  /// Tab keeps the highlighted transform as a step; Backspace on an empty
  /// filter removes the last step.
  KeyEventResult _onListKey(FocusNode node, KeyEvent event) {
    if (event is! KeyDownEvent) return KeyEventResult.ignored;
    final keyboard = HardwareKeyboard.instance;
    if (event.logicalKey == LogicalKeyboardKey.tab &&
        !keyboard.isShiftPressed) {
      _controller.commitCandidate();
      return KeyEventResult.handled;
    }
    if (event.logicalKey == LogicalKeyboardKey.backspace &&
        (_controller.query ?? '').isEmpty &&
        _controller.removeLastStep()) {
      return KeyEventResult.handled;
    }
    return KeyEventResult.ignored;
  }

  void _back() {
    if (_paramsScope.hasFocus) {
      _focusFirst(_listScope);
    } else if (_controller.selectedStep != null) {
      _controller.selectStep(null);
    } else {
      widget.onDone(null);
    }
  }

  static void _focusFirst(FocusScopeNode scope) =>
      scope.traversalDescendants.firstOrNull?.requestFocus();

  Future<void> _savePipeline() async {
    if (!_controller.canSavePipeline) return;
    var name = '';
    final changes = ValueNotifier(0);
    try {
      final saved = await AppOverlays.showActionDialog(
        context,
        icon: LucideIcons.workflow,
        title: Text(context.l10n.transformsSavePipelineTitle),
        actionLabel: context.l10n.commonSave,
        destructive: false,
        changes: changes,
        canConfirm: () =>
            name.trim().isNotEmpty &&
            name.trim().length <= TransformController.maxPipelineName &&
            !_controller.pipelineNameTaken(name),
        failureMessage: () => context.l10n.transformsSavePipelineFailed,
        onConfirm: () async {
          if (await _controller.savePipeline(name) == null) {
            throw StateError('pipeline not saved');
          }
          return true;
        },
        content: Builder(
          builder: (context) => Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              TextField(
                key: const ValueKey('transform-pipeline-name'),
                autofocus: true,
                maxLength: TransformController.maxPipelineName,
                decoration: AppOverlays.dialogFieldDecoration(context),
                placeholder: Text(context.l10n.transformsPipelineNameLabel),
                onChanged: (value) {
                  name = value;
                  changes.value++;
                },
              ),
              ValueListenableBuilder(
                valueListenable: changes,
                builder: (context, _, _) => _controller.pipelineNameTaken(name)
                    ? Padding(
                        padding: const EdgeInsets.only(top: AppSpacing.xs),
                        child: Text(
                          context.l10n.transformsPipelineNameTaken,
                        ).small().muted(),
                      )
                    : const SizedBox.shrink(),
              ),
            ],
          ),
        ),
      );
      if (saved == true && mounted) {
        AppToast.show(
          context,
          title: context.l10n.transformsPipelineSavedTitle,
          message: context.l10n.transformsPipelineSavedMessage(name.trim()),
          tone: AppToastTone.success,
        );
      }
    } finally {
      changes.dispose();
    }
  }

  @override
  Widget build(BuildContext context) => Focus(
    onKeyEvent: _onKey,
    skipTraversal: true,
    canRequestFocus: false,
    child: ListenableBuilder(
      listenable: _controller,
      builder: (context, _) {
        final l10n = context.l10n;
        final ready = _controller.status == TransformPaletteStatus.ready;
        return AppOverlays.alertDialog(
          icon: LucideIcons.wandSparkles,
          title: Text(l10n.transformsPaletteTitle),
          content: SizedBox(
            key: const ValueKey('transform-palette'),
            height: math.max(
              MediaQuery.sizeOf(context).height *
                  AppOverlaySize.dialogContentHeightFactor,
              AppOverlaySize.selectMaxHeight,
            ),
            child: DefaultTextStyle.merge(
              style: TextStyle(color: Theme.of(context).colorScheme.foreground),
              child: switch (_controller.status) {
                TransformPaletteStatus.loading => const StateView.loading(),
                TransformPaletteStatus.notText => StateView.empty(
                  key: const ValueKey('transform-not-text'),
                  icon: LucideIcons.fileType,
                  title: l10n.transformsNotTextTitle,
                  message: l10n.transformsNotTextMessage,
                ),
                TransformPaletteStatus.empty => StateView.empty(
                  icon: LucideIcons.clipboard,
                  title: l10n.transformsNothingTitle,
                  message: l10n.transformsNothingMessage,
                ),
                TransformPaletteStatus.failed => StateView.error(
                  title: l10n.transformsLoadFailed,
                  message: _controller.loadError,
                ),
                TransformPaletteStatus.ready => _body(context),
              },
            ),
          ),
          actions: [
            if (!ready)
              Button.secondary(
                style: AppTheme.dialogButtonStyle(
                  const ButtonStyle.secondary(),
                ),
                onPressed: () => widget.onDone(null),
                child: Text(l10n.commonClose),
              )
            else ...[
              _action(
                context,
                key: 'transform-save',
                label: l10n.transformsActionSave,
                icon: LucideIcons.save,
                hint: 'S',
                enabled: _controller.canSave,
                running: _controller.running == TransformRunAction.saveNewClip,
                onPressed: () => unawaited(_act(_controller.saveNewClip)),
              ),
              _action(
                context,
                key: 'transform-copy',
                label: l10n.commonCopy,
                icon: LucideIcons.copy,
                hint: 'C',
                primary: !_controller.canPasteTarget,
                enabled: _controller.canCopy,
                running: _controller.running == TransformRunAction.copy,
                onPressed: () => unawaited(_act(_controller.copy)),
              ),
              if (_controller.canPasteTarget)
                _action(
                  context,
                  key: 'transform-paste',
                  label: l10n.transformsActionPaste,
                  icon: LucideIcons.clipboardPaste,
                  primary: true,
                  enabled: _controller.canPaste,
                  running: _controller.running == TransformRunAction.paste,
                  onPressed: () => unawaited(_act(_controller.paste)),
                ),
            ],
          ],
        );
      },
    ),
  );

  Widget _action(
    BuildContext context, {
    required String key,
    required String label,
    required IconData icon,
    required bool enabled,
    required bool running,
    required VoidCallback onPressed,
    String? hint,
    bool primary = false,
  }) {
    final style = primary
        ? const ButtonStyle.primary()
        : const ButtonStyle.secondary();
    return Button(
      key: ValueKey(key),
      style: AppTheme.dialogButtonStyle(style),
      onPressed: enabled ? onPressed : null,
      leading: running
          ? const AppIconFrame(
              size: AppIconSize.sm,
              child: CircularProgressIndicator(),
            )
          : AppIconFrame(child: Icon(icon)),
      child: Text(label),
    );
  }

  Widget _body(BuildContext context) => LayoutBuilder(
    builder: (context, constraints) {
      final wide = constraints.maxWidth >= AdaptiveBreakpoints.compactOverlay;
      final list = _list(context);
      final details = _details(context);
      if (wide) {
        return Row(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Expanded(flex: 5, child: list),
            const Gap(AppSpacing.md),
            Expanded(flex: 6, child: details),
          ],
        );
      }
      return Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          SizedBox(
            height:
                constraints.maxHeight *
                AppOverlaySize.dialogContentHeightFactor,
            child: list,
          ),
          const Gap(AppSpacing.md),
          Expanded(child: details),
        ],
      );
    },
  );

  Widget _list(BuildContext context) => FocusScope(
    node: _listScope,
    onKeyEvent: _onListKey,
    child: Command(
      debounceDuration: Duration.zero,
      searchPlaceholder: Text(context.l10n.transformsFilterPlaceholder),
      emptyBuilder: (context) => StateView.empty(
        compact: true,
        icon: LucideIcons.searchX,
        title: context.l10n.transformsNoMatches,
      ),
      builder: (context, query) {
        _controller.setQuery(query);
        _highlight.release();
        if (_controller.groups(query).isEmpty) return Stream.value(const []);
        return Stream.value([
          _PaletteList(
            key: ValueKey(query),
            controller: _controller,
            query: query,
            tracker: _highlight,
            onActivate: () => _act(_controller.runPrimary),
          ),
        ]);
      },
    ),
  );

  Widget _details(BuildContext context) {
    final l10n = context.l10n;
    final paramIndex = _controller.paramStepIndex;
    final paramDefinition = paramIndex == null
        ? null
        : _controller.definition(_controller.chain[paramIndex].transformId);
    final params = paramDefinition?.params ?? const <TransformParam>[];
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _ChainStrip(
          controller: _controller,
          onSavePipeline: _controller.canSavePipeline
              ? () => unawaited(_savePipeline())
              : null,
        ),
        if (params.isNotEmpty && paramIndex != null) ...[
          const Gap(AppSpacing.sm),
          ConstrainedBox(
            constraints: const BoxConstraints(
              maxHeight: AppOverlaySize.editorBodyMinHeight,
            ),
            child: FocusScope(
              node: _paramsScope,
              child: SingleChildScrollView(
                child: TransformParamFields(
                  keyPrefix: 'transform-param-$paramIndex',
                  params: params,
                  enabled: _controller.stepEditable(paramIndex),
                  valueOf: (param) => _controller.paramValue(paramIndex, param),
                  onChanged: (param, value) =>
                      _controller.setParam(paramIndex, param, value),
                ),
              ),
            ),
          ),
        ],
        const Gap(AppSpacing.sm),
        Expanded(child: _previewPane(context)),
        if (_controller.actionError case final error?) ...[
          const Gap(AppSpacing.sm),
          Semantics(
            liveRegion: true,
            child: Alert.destructive(
              key: const ValueKey('transform-action-error'),
              leading: const AppIconFrame(child: Icon(LucideIcons.circleAlert)),
              title: Text(l10n.appActionFailed),
              content: Text(error),
            ),
          ),
        ],
      ],
    );
  }

  Widget _previewPane(BuildContext context) {
    final l10n = context.l10n;
    final controller = _controller;
    final preview = controller.preview;
    final shown = controller.shownStep;
    if (controller.chain.isEmpty ||
        (preview == null && !controller.previewPending)) {
      if (controller.previewError case final error?) {
        return _previewError(context, error);
      }
      return StateView.empty(
        compact: true,
        icon: LucideIcons.wandSparkles,
        title: l10n.transformsPickTransform,
        message: _desktop ? l10n.transformsPickTransformHint : null,
      );
    }
    final failure = preview?.failure;
    final failedIndex = preview?.failedStep;
    final selected = controller.selectedStep;
    final showsNotRun =
        shown != null &&
        shown.notRun &&
        shown.failure == null &&
        (selected != null && failedIndex != null && selected > failedIndex);
    final definition = shown == null || preview == null
        ? null
        : controller.definition(
            controller
                .chain[math.min(
                  selected ?? preview.steps.length - 1,
                  controller.chain.length - 1,
                )]
                .transformId,
          );
    final monospace = definition?.category.monospace ?? false;
    final status = _statusLine(context, preview, shown);
    final body = <Widget>[];
    if (controller.previewError case final error?) {
      body.add(_previewError(context, error));
    } else if (failure != null &&
        failedIndex != null &&
        failedIndex < controller.chain.length &&
        (selected == null || selected >= failedIndex)) {
      final stepName =
          controller
              .definition(controller.chain[failedIndex].transformId)
              ?.name ??
          controller.chain[failedIndex].transformId;
      body.add(
        Semantics(
          liveRegion: true,
          child: Alert.destructive(
            key: const ValueKey('transform-preview-error'),
            leading: const AppIconFrame(child: Icon(LucideIcons.circleAlert)),
            title: Text(
              l10n.transformsStepFailed(
                failedIndex + 1,
                stepName,
                l10n.transformIssue(failure),
              ),
            ),
            trailing: controller.canUseHexOutput
                ? Button.secondary(
                    style: AppTheme.controlButtonStyle(
                      const ButtonStyle.secondary(),
                    ),
                    onPressed: controller.useHexOutput,
                    child: Text(l10n.transformsUseHexOutput),
                  )
                : null,
          ),
        ),
      );
    }
    if (shown != null && shown.warnings.isNotEmpty && failure == null) {
      body.add(
        Alert(
          key: const ValueKey('transform-preview-warning'),
          theme: AppTheme.statusAlertTheme(context, AppStatusTone.warning),
          leading: AppIconFrame(
            child: Icon(
              LucideIcons.triangleAlert,
              color: AppTheme.statusColor(context, AppStatusTone.warning),
            ),
          ),
          title: Text(
            {
              for (final warning in shown.warnings)
                l10n.transformWarning(warning),
            }.join(' · '),
          ),
        ),
      );
    }
    final text = switch (_pane) {
      _PreviewPane.input => _inputOf(selected) ?? '',
      _ => shown?.output ?? '',
    };
    Widget content;
    if (showsNotRun) {
      content = Center(child: Text(l10n.transformsNotRun).muted());
    } else if (controller.masked) {
      content = Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(l10n.transformsSensitiveHidden).muted(),
            const Gap(AppSpacing.sm),
            Button.secondary(
              key: const ValueKey('transform-reveal'),
              style: AppTheme.controlButtonStyle(const ButtonStyle.secondary()),
              onPressed: controller.reveal,
              leading: const AppIconFrame(child: Icon(LucideIcons.eye)),
              child: Text(l10n.transformsReveal),
            ),
          ],
        ),
      );
    } else if (_pane == _PreviewPane.changes && _desktop) {
      content = SingleChildScrollView(
        child: _ChangesText(
          before: _inputOf(selected) ?? '',
          after: shown?.output ?? '',
          monospace: monospace,
        ),
      );
    } else {
      final style = monospace
          ? Theme.of(
              context,
            ).typography.mono.merge(Theme.of(context).typography.small)
          : Theme.of(context).typography.small;
      content = SingleChildScrollView(
        child: SelectableText(
          text,
          key: const ValueKey('transform-preview-output'),
          style: style,
        ),
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          children: [
            Expanded(
              child: Semantics(
                liveRegion: true,
                child: Text(
                  status,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ).small().muted(),
              ),
            ),
            if (controller.showSpinner) ...[
              const Gap(AppSpacing.sm),
              const AppIconFrame(
                size: AppIconSize.sm,
                child: CircularProgressIndicator(),
              ),
            ],
            if (controller.showCancel) ...[
              const Gap(AppSpacing.sm),
              Button.ghost(
                key: const ValueKey('transform-preview-cancel'),
                style: AppTheme.controlButtonStyle(const ButtonStyle.ghost()),
                onPressed: controller.cancelPreview,
                child: Text(context.l10n.commonCancel),
              ),
            ],
            if (_desktop && preview?.input != null) ...[
              const Gap(AppSpacing.sm),
              Tabs(
                index: _pane.index,
                onChanged: (index) =>
                    setState(() => _pane = _PreviewPane.values[index]),
                children: [
                  TabItem(child: Text(l10n.transformsPaneOutput)),
                  TabItem(child: Text(l10n.transformsPaneInput)),
                  TabItem(child: Text(l10n.transformsPaneChanges)),
                ],
              ),
            ],
          ],
        ),
        const Gap(AppSpacing.xs),
        for (final widget in body) ...[widget, const Gap(AppSpacing.xs)],
        Expanded(
          child: OutlinedContainer(
            padding: const EdgeInsets.all(AppSpacing.sm),
            child: SizedBox(width: double.infinity, child: content),
          ),
        ),
      ],
    );
  }

  /// The text that step [selected] read: the previous step's output, or
  /// the source for the first step.
  String? _inputOf(int? selected) {
    final preview = _controller.preview;
    if (preview == null) return null;
    final index = selected ?? preview.steps.length - 1;
    if (index <= 0) return preview.input;
    if (index > preview.steps.length) return null;
    return preview.steps[index - 1].output;
  }

  Widget _previewError(BuildContext context, String message) => Semantics(
    liveRegion: true,
    child: Alert.destructive(
      key: const ValueKey('transform-preview-error'),
      leading: const AppIconFrame(child: Icon(LucideIcons.circleAlert)),
      title: Text(context.l10n.transformsCannotTransform),
      content: Text(message),
    ),
  );

  String _statusLine(
    BuildContext context,
    TransformPreview? preview,
    TransformStepPreview? shown,
  ) {
    final l10n = context.l10n;
    if (preview == null || shown == null) return l10n.transformsPreviewing;
    final parts = <String>[
      l10n.transformsCounts(
        shown.counts.lines,
        shown.counts.words,
        shown.counts.chars,
      ),
      if (preview.noChanges && _controller.selectedStep == null)
        l10n.transformsNoChanges,
      if (preview.truncated)
        l10n.transformsTruncated(formatBytes(preview.inputCounts.bytes)),
      if (_controller.chain.any(
        (step) =>
            !(_controller.definition(step.transformId)?.deterministic ?? true),
      ))
        l10n.transformsRandom,
    ];
    return parts.join(' · ');
  }
}

/// The row that holds the list highlight.
///
/// The list moves the highlight to a sibling when the highlighted row is
/// removed, which marks widgets dirty while the tree is locked. Releasing
/// the highlight before the rows change avoids that; the first row of the
/// new list then takes it.
class _HighlightTracker {
  SubFocusState? _row;

  void track(SubFocusState row) => _row = row;

  void release() {
    final row = _row;
    _row = null;
    if (row == null || row is! State || !(row as State).mounted) return;
    if (row.isFocused) row.unfocus();
  }
}

/// The list inside [Command]: one widget so it follows controller changes
/// (suggestions, favorites, new pipelines) without resetting the filter.
class _PaletteList extends StatefulWidget {
  const _PaletteList({
    super.key,
    required this.controller,
    required this.query,
    required this.tracker,
    required this.onActivate,
  });

  final TransformController controller;
  final String? query;
  final _HighlightTracker tracker;
  final Future<void> Function() onActivate;

  @override
  State<_PaletteList> createState() => _PaletteListState();
}

class _PaletteListState extends State<_PaletteList> {
  late List<TransformEntryGroup> _groups;
  late int _rows;

  TransformController get _controller => widget.controller;

  @override
  void initState() {
    super.initState();
    _groups = _controller.groups(widget.query);
    _rows = _signature(_groups);
    _controller.addListener(_changed);
  }

  @override
  void dispose() {
    _controller.removeListener(_changed);
    super.dispose();
  }

  static int _signature(List<TransformEntryGroup> groups) => Object.hashAll([
    for (final group in groups)
      for (final entry in group.entries) '${group.kind.name}:${entry.id}',
  ]);

  /// New rows replace the list after the highlight is released; a fresh
  /// list highlights its first row, such as a suggestion that just arrived.
  void _changed() {
    final groups = _controller.groups(widget.query);
    final rows = _signature(groups);
    if (rows != _rows) widget.tracker.release();
    setState(() {
      _groups = groups;
      _rows = rows;
    });
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final favorites = _controller.favoriteEntries;
    return Column(
      key: ValueKey(_rows),
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        for (final group in _groups)
          CommandCategory(
            title: Text(switch (group.kind) {
              TransformEntryGroupKind.results => l10n.transformsGroupResults,
              TransformEntryGroupKind.suggested =>
                l10n.transformsGroupSuggested,
              TransformEntryGroupKind.recent => l10n.transformsGroupRecent,
              TransformEntryGroupKind.favorites =>
                l10n.transformsGroupFavorites,
              TransformEntryGroupKind.pipelines =>
                l10n.transformsGroupPipelines,
              TransformEntryGroupKind.category => l10n.transformCategory(
                group.category!,
              ),
            }),
            children: [
              for (final entry in group.entries)
                _PaletteItem(
                  key: ValueKey(
                    'transform-entry-${group.kind.name}-${entry.id}',
                  ),
                  controller: _controller,
                  entry: entry,
                  tracker: widget.tracker,
                  onActivate: widget.onActivate,
                  favoriteIndex: group.kind == TransformEntryGroupKind.favorites
                      ? favorites.indexOf(entry)
                      : -1,
                ),
            ],
          ),
      ],
    );
  }
}

/// A palette row. It joins the [Command] list's sub-focus so arrow keys move
/// the highlight, and reports each highlight change so the preview follows
/// it; Enter runs the primary action. Hovering does not move the highlight,
/// so reaching an action button with the pointer keeps the preview.
class _PaletteItem extends StatefulWidget {
  const _PaletteItem({
    super.key,
    required this.controller,
    required this.entry,
    required this.favoriteIndex,
    required this.tracker,
    required this.onActivate,
  });

  final TransformController controller;
  final TransformEntry entry;
  final int favoriteIndex;
  final _HighlightTracker tracker;
  final Future<void> Function() onActivate;

  @override
  State<_PaletteItem> createState() => _PaletteItemState();
}

class _PaletteItemState extends State<_PaletteItem> {
  bool _wasFocused = false;

  void _report() => WidgetsBinding.instance.addPostFrameCallback((_) {
    if (mounted) widget.controller.highlight(widget.entry);
  });

  @override
  Widget build(BuildContext context) {
    final controller = widget.controller;
    final entry = widget.entry;
    final theme = Theme.of(context);
    final l10n = context.l10n;
    final description = switch (entry.kind) {
      _ when entry.unavailable => l10n.transformsNeedsNewerVersion,
      TransformEntryKind.transform => entry.definition!.description,
      TransformEntryKind.pipeline =>
        entry.pipeline!.description ??
            l10n.transformsPipelineSteps(entry.pipeline!.steps.length),
      TransformEntryKind.recent => l10n.transformsGroupRecent,
    };
    final icon = switch (entry.kind) {
      TransformEntryKind.transform => LucideIcons.wandSparkles,
      TransformEntryKind.pipeline => LucideIcons.workflow,
      TransformEntryKind.recent => LucideIcons.history,
    };
    final name = controller.entryName(entry);
    final shortcut =
        widget.favoriteIndex >= 0 &&
            widget.favoriteIndex < TransformController.maxFavoriteShortcuts
        ? '${defaultTargetPlatform == TargetPlatform.macOS ? '⌘' : 'Ctrl+'}'
              '${widget.favoriteIndex + 1}'
        : null;
    return Actions(
      actions: {
        ActivateIntent: CallbackAction<Intent>(
          // Enter runs the chain as it stands; after Tab that is the kept
          // steps without the still-highlighted row.
          onInvoke: (_) {
            if (!entry.unavailable) unawaited(widget.onActivate());
            return null;
          },
        ),
      },
      child: SubFocus(
        enabled: !entry.unavailable,
        builder: (context, state) {
          final focused = state.isFocused;
          if (focused) widget.tracker.track(state);
          if (focused && !_wasFocused) _report();
          _wasFocused = focused;
          return Semantics(
            selected: focused,
            button: true,
            enabled: !entry.unavailable,
            label: entry.kind == TransformEntryKind.transform
                ? '$name, ${l10n.transformCategory(entry.definition!.category)}'
                : name,
            hint: description,
            child: Clickable(
              enabled: !entry.unavailable,
              onPressed: () {
                state.requestFocus();
                controller.highlight(entry);
              },
              child: AnimatedContainer(
                duration: kDefaultDuration,
                decoration: BoxDecoration(
                  color: focused
                      ? theme.colorScheme.accent
                      : theme.colorScheme.accent.withValues(alpha: 0),
                  borderRadius: BorderRadius.circular(theme.radiusSm),
                ),
                padding: const EdgeInsets.symmetric(
                  horizontal: AppSpacing.sm,
                  vertical: AppSpacing.xs,
                ),
                child: ExcludeSemantics(
                  child: Row(
                    children: [
                      AppIconFrame(
                        size: AppIconSize.sm,
                        child: Icon(icon, size: AppIconSize.sm),
                      ).iconMutedForeground(),
                      const Gap(AppSpacing.sm),
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(
                              name,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                            ).small().medium(),
                            Text(
                              description,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                            ).xSmall().muted(),
                          ],
                        ),
                      ),
                      if (controller.isFavorite(entry)) ...[
                        const Gap(AppSpacing.xs),
                        const AppIconFrame(
                          size: AppIconSize.xs,
                          child: Icon(LucideIcons.star, size: AppIconSize.xs),
                        ).iconMutedForeground(),
                      ],
                      if (shortcut != null) ...[
                        const Gap(AppSpacing.xs),
                        Text(shortcut).xSmall().muted(),
                      ],
                    ],
                  ),
                ),
              ),
            ),
          );
        },
      ),
    );
  }
}

/// The chain: committed steps, the highlighted candidate, and controls for
/// the selected step.
class _ChainStrip extends StatelessWidget {
  const _ChainStrip({required this.controller, this.onSavePipeline});

  final TransformController controller;
  final VoidCallback? onSavePipeline;

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final chain = controller.chain;
    final committed = controller.committedCount;
    final selected = controller.selectedStep;
    String nameOf(TransformStep step) =>
        controller.definition(step.transformId)?.name ?? step.transformId;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          children: [
            Expanded(
              child: Wrap(
                spacing: AppSpacing.xs,
                runSpacing: AppSpacing.xs,
                crossAxisAlignment: WrapCrossAlignment.center,
                children: [
                  for (var index = 0; index < chain.length; index++)
                    Button(
                      key: ValueKey('transform-step-$index'),
                      style: AppTheme.controlButtonStyle(
                        selected == index
                            ? const ButtonStyle.primary()
                            : index < committed
                            ? const ButtonStyle.secondary()
                            : const ButtonStyle.outline(),
                      ),
                      onPressed: () => controller.selectStep(
                        selected == index ? null : index,
                      ),
                      child: Text(
                        index < committed
                            ? l10n.transformsStepLabel(
                                index + 1,
                                nameOf(chain[index]),
                              )
                            : l10n.transformsThenLabel(nameOf(chain[index])),
                        style: chain[index].enabled
                            ? null
                            : const TextStyle(
                                decoration: TextDecoration.lineThrough,
                              ),
                      ),
                    ),
                  if (chain.isEmpty)
                    Text(l10n.transformsChainEmpty).small().muted(),
                ],
              ),
            ),
            if (onSavePipeline != null)
              Tooltip(
                tooltip: (context) =>
                    TooltipContainer(child: Text(l10n.transformsSavePipeline)),
                child: Button.ghost(
                  key: const ValueKey('transform-save-pipeline'),
                  style: AppTheme.controlButtonStyle(
                    const ButtonStyle.ghostIcon(),
                  ),
                  onPressed: onSavePipeline,
                  child: Semantics(
                    label: l10n.transformsSavePipeline,
                    child: const AppIconFrame(
                      child: Icon(LucideIcons.workflow),
                    ),
                  ),
                ),
              ),
          ],
        ),
        if (selected != null && selected < committed) ...[
          const Gap(AppSpacing.xs),
          Row(
            children: [
              MergeSemantics(
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Switch(
                      key: const ValueKey('transform-step-enabled'),
                      value: chain[selected].enabled,
                      onChanged: (value) =>
                          controller.setStepEnabled(selected, value),
                    ),
                    const Gap(AppSpacing.xs),
                    Text(l10n.transformsStepEnabled).small(),
                  ],
                ),
              ),
              const Spacer(),
              _iconButton(
                context,
                key: 'transform-step-earlier',
                label: l10n.transformsMoveEarlier,
                icon: LucideIcons.arrowUp,
                onPressed: selected > 0
                    ? () => controller.moveStep(selected, -1)
                    : null,
              ),
              _iconButton(
                context,
                key: 'transform-step-later',
                label: l10n.transformsMoveLater,
                icon: LucideIcons.arrowDown,
                onPressed: selected < committed - 1
                    ? () => controller.moveStep(selected, 1)
                    : null,
              ),
              _iconButton(
                context,
                key: 'transform-step-remove',
                label: l10n.transformsRemoveStep,
                icon: LucideIcons.x,
                onPressed: () => controller.removeStep(selected),
              ),
            ],
          ),
        ],
      ],
    );
  }

  Widget _iconButton(
    BuildContext context, {
    required String key,
    required String label,
    required IconData icon,
    required VoidCallback? onPressed,
  }) => Tooltip(
    tooltip: (context) => TooltipContainer(child: Text(label)),
    child: Button.ghost(
      key: ValueKey(key),
      style: AppTheme.controlButtonStyle(const ButtonStyle.ghostIcon()),
      onPressed: onPressed,
      child: Semantics(
        label: label,
        child: AppIconFrame(child: Icon(icon)),
      ),
    ),
  );
}

/// Highlights the span that differs between [before] and [after]: the text
/// between their common prefix and common suffix.
class _ChangesText extends StatelessWidget {
  const _ChangesText({
    required this.before,
    required this.after,
    required this.monospace,
  });

  final String before;
  final String after;
  final bool monospace;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    var start = 0;
    final shortest = math.min(before.length, after.length);
    while (start < shortest &&
        before.codeUnitAt(start) == after.codeUnitAt(start)) {
      start++;
    }
    var end = 0;
    while (end < shortest - start &&
        before.codeUnitAt(before.length - 1 - end) ==
            after.codeUnitAt(after.length - 1 - end)) {
      end++;
    }
    final style = monospace
        ? theme.typography.mono.merge(theme.typography.small)
        : theme.typography.small;
    return SelectableText.rich(
      key: const ValueKey('transform-preview-changes'),
      TextSpan(
        style: style,
        children: [
          TextSpan(text: after.substring(0, start)),
          TextSpan(
            text: after.substring(start, after.length - end),
            style: TextStyle(
              backgroundColor: AppTheme.statusColor(
                context,
                AppStatusTone.info,
              ).withValues(alpha: 0.24),
            ),
          ),
          TextSpan(text: after.substring(after.length - end)),
        ],
      ),
    );
  }
}
