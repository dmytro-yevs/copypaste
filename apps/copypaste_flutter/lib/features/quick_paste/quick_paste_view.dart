import 'dart:async';
import 'dart:io';

import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../app/theme/app_overlays.dart';
import '../../app/theme/app_theme.dart';
import '../../app/theme/app_tokens.dart';
import '../../shared/state_view.dart';
import '../history/controller/history_controller.dart';
import '../history/models/history_models.dart';
import '../history/view/history_inspector.dart';
import '../history/presentation/history_clip_presentation.dart';
import '../history/presentation/source_app_label.dart';
import 'quick_paste_controller.dart';

class QuickPasteView extends StatefulWidget {
  const QuickPasteView({super.key, required this.controller});

  final QuickPasteController controller;

  @override
  State<QuickPasteView> createState() => _QuickPasteViewState();
}

class _FocusClipIntent extends Intent {
  const _FocusClipIntent();
}

class _QuickPasteViewState extends State<QuickPasteView> {
  final _search = TextEditingController();
  final _searchFocus = FocusNode();
  final _scroll = ScrollController();
  int _generation = 0;

  QuickPasteController get controller => widget.controller;

  @override
  void initState() {
    super.initState();
    _generation = controller.presentationGeneration;
    controller.addListener(_presentationChanged);
  }

  void _presentationChanged() {
    if (_generation == controller.presentationGeneration) return;
    _generation = controller.presentationGeneration;
    _search.clear();
    _searchFocus.requestFocus();
    if (_scroll.hasClients) _scroll.jumpTo(0);
  }

  @override
  void dispose() {
    controller.removeListener(_presentationChanged);
    _search.dispose();
    _searchFocus.dispose();
    _scroll.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: controller,
      builder: (context, child) => SubFocusScope(
        key: ValueKey(controller.presentationGeneration),
        builder: (context, scope) => CallbackShortcuts(
          bindings: _shortcutBindings(context, scope),
          child: ClipRRect(
            borderRadius: const BorderRadius.all(Radius.circular(AppRadius.lg)),
            child: Scaffold(
              backgroundColor: Theme.of(context).colorScheme.popover,
              child: Padding(
                padding: const EdgeInsets.all(AppSpacing.sm),
                child: LayoutBuilder(
                  builder: (context, constraints) {
                    final menu = _menu(context);
                    if (!controller.inspectorOpen) return menu;
                    final menuWidth =
                        (AppLayoutSize.quickPasteMenuWidth - AppSpacing.sm * 2)
                            .clamp(0.0, constraints.maxWidth * 0.55)
                            .toDouble();
                    return Row(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        SizedBox(width: menuWidth, child: menu),
                        const Gap(AppSpacing.sm),
                        Expanded(
                          child: HistoryInspector(
                            controller: controller.history,
                            inDrawer: false,
                            showActions: false,
                            compact: true,
                            onClose: () =>
                                unawaited(controller.toggleInspector()),
                          ),
                        ),
                      ],
                    );
                  },
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }

  Widget _menu(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      Row(
        key: const ValueKey('quick-paste-header'),
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          const Image(
            key: ValueKey('quick-paste-logo'),
            image: AssetImage('assets/brand/copypaste.png'),
            width: AppIconSize.sm,
            height: AppIconSize.sm,
            semanticLabel: 'CopyPaste',
          ),
          const Gap(AppSpacing.xs),
          Expanded(
            child: Theme(
              data: AppTheme.clipboardSearchTheme(context),
              child: SizedBox(
                height: AppControlSize.compact,
                child: TextField(
                  theme: AppTheme.clipboardSearchFieldTheme,
                  textAlignVertical: TextAlignVertical.center,
                  key: const ValueKey('quick-paste-search'),
                  controller: _search,
                  focusNode: _searchFocus,
                  autofocus: true,
                  style: AppTheme.clipboardMenuTextStyle(context),
                  placeholder: Text(
                    'Type to search…',
                    style: AppTheme.clipboardMenuTextStyle(context),
                  ),
                  features: const [
                    InputFeature.leading(
                      Icon(LucideIcons.search, size: AppIconSize.sm),
                    ),
                  ],
                  onChanged: (value) => unawaited(controller.search(value)),
                ),
              ),
            ),
          ),
          const Gap(AppSpacing.xs),
          Button.ghost(
            style: const ButtonStyle.ghostIcon(),
            key: const ValueKey('quick-paste-inspector-toggle'),
            onPressed: controller.toggleInspector,
            child: const Icon(LucideIcons.panelsLeftBottom),
          ),
        ],
      ),
      const Gap(AppSpacing.xs),
      Expanded(child: _history(context)),
      const Gap(AppSpacing.xs),
      const Divider(),
      const Gap(AppSpacing.xs),
      _Footer(controller: controller),
    ],
  );

  Widget _history(BuildContext context) {
    final history = controller.history;
    if (history.state == HistoryLoadState.loading && controller.items.isEmpty) {
      return const StateView.loading(message: 'Loading clipboard history.');
    }
    if (history.state == HistoryLoadState.error) {
      return StateView.error(
        title: 'History is unavailable',
        message: history.errorMessage,
      );
    }
    final items = controller.items;
    if (items.isEmpty) {
      return const StateView.empty(title: 'No clips found');
    }
    return Scrollbar(
      controller: _scroll,
      child: SingleChildScrollView(
        controller: _scroll,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            for (final (index, clip) in items.indexed) ...[
              if (index > 0 && clip.pinned && !items[index - 1].pinned) ...[
                const Gap(AppSpacing.xs),
                const Divider(),
                const Gap(AppSpacing.xs),
              ],
              _item(clip),
            ],
          ],
        ),
      ),
    );
  }

  Map<ShortcutActivator, VoidCallback> _shortcutBindings(
    BuildContext context,
    SubFocusScopeState scope,
  ) {
    void move(TraversalDirection direction, [int count = 1]) {
      for (var step = 0; step < count; step++) {
        scope.nextFocus(direction);
      }
      scope.invokeActionOnFocused(const _FocusClipIntent());
    }

    final bindings = <ShortcutActivator, VoidCallback>{
      const SingleActivator(LogicalKeyboardKey.escape): () =>
          unawaited(controller.close()),
      const SingleActivator(LogicalKeyboardKey.arrowDown): () =>
          move(TraversalDirection.down),
      const SingleActivator(LogicalKeyboardKey.arrowUp): () =>
          move(TraversalDirection.up),
      const SingleActivator(LogicalKeyboardKey.pageDown): () =>
          move(TraversalDirection.down, 5),
      const SingleActivator(LogicalKeyboardKey.pageUp): () =>
          move(TraversalDirection.up, 5),
      const SingleActivator(LogicalKeyboardKey.enter): () =>
          unawaited(controller.activateFocused()),
      const SingleActivator(LogicalKeyboardKey.enter, shift: true): () =>
          unawaited(controller.activateFocused(plainText: true)),
      const SingleActivator(LogicalKeyboardKey.enter, alt: true): () =>
          unawaited(controller.activateFocused(invertAutoPaste: true)),
      const SingleActivator(
        LogicalKeyboardKey.enter,
        alt: true,
        shift: true,
      ): () => unawaited(
        controller.activateFocused(plainText: true, forcePaste: true),
      ),
      const SingleActivator(LogicalKeyboardKey.keyP, alt: true): () =>
          unawaited(controller.toggleFocusedPin()),
      const SingleActivator(LogicalKeyboardKey.backspace, alt: true): () =>
          unawaited(controller.deleteFocused()),
      const SingleActivator(LogicalKeyboardKey.delete, alt: true): () =>
          unawaited(controller.deleteFocused()),
      _desktopShortcut(LogicalKeyboardKey.comma): () =>
          unawaited(controller.openSettings()),
      _desktopShortcut(LogicalKeyboardKey.keyQ): () =>
          unawaited(controller.quit()),
    };
    for (final entry in controller.shortcuts.entries) {
      bindings[_desktopShortcut(entry.key)] = () =>
          unawaited(controller.activate(entry.value));
    }
    return bindings;
  }

  Widget _item(HistoryClip clip) {
    final shortcut = controller.shortcutFor(clip);
    return Actions(
      actions: {
        _FocusClipIntent: CallbackAction<_FocusClipIntent>(
          onInvoke: (_) {
            controller.focus(clip);
            return null;
          },
        ),
      },
      child: SubFocus(
        key: ValueKey('quick-paste-${clip.id}'),
        builder: (context, focus) => Button.ghost(
          style: AppTheme.clipboardMenuButtonStyle(selected: focus.isFocused),
          alignment: Alignment.centerLeft,
          onHover: (hovered) {
            if (hovered) {
              focus.requestFocus();
              controller.focus(clip);
            }
          },
          onFocus: (focused) {
            if (focused) {
              focus.requestFocus();
              controller.focus(clip);
            }
          },
          onPressed: controller.activating
              ? null
              : () {
                  final keyboard = HardwareKeyboard.instance;
                  unawaited(
                    controller.activate(
                      clip,
                      plainText: keyboard.isShiftPressed,
                      invertAutoPaste:
                          keyboard.isAltPressed && !keyboard.isShiftPressed,
                      forcePaste:
                          keyboard.isAltPressed && keyboard.isShiftPressed,
                    ),
                  );
                },
          child: Row(
            children: [
              SizedBox(
                width: AppIconSize.sm,
                child: clip.sourceApp != null
                    ? SourceAppLabel(
                        name: clip.sourceApp!,
                        icon: controller.history.requestSourceIcon(clip.id),
                        showName: false,
                      )
                    : Icon(
                        HistoryClipPresentation.icon(clip.contentKind),
                        size: AppIconSize.sm,
                      ),
              ),
              const Gap(AppSpacing.sm),
              Expanded(
                child: _ClipContent(clip: clip, controller: controller),
              ),
              const Gap(AppSpacing.sm),
              if (shortcut != null)
                MenuShortcut(
                  activator: _desktopShortcut(shortcut),
                  combiner: const SizedBox.shrink(),
                ),
            ],
          ),
        ),
      ),
    );
  }
}

SingleActivator _desktopShortcut(LogicalKeyboardKey key) =>
    SingleActivator(key, meta: Platform.isMacOS, control: Platform.isWindows);

class _ClipContent extends StatelessWidget {
  const _ClipContent({required this.clip, required this.controller});

  final HistoryClip clip;
  final QuickPasteController controller;

  @override
  Widget build(BuildContext context) {
    if (clip.contentKind != HistoryClipKind.image) {
      return Text(
        clip.preview.isEmpty ? clip.contentKind.label : clip.preview,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
      );
    }
    return FutureBuilder<HistoryImagePreview?>(
      future: controller.history.requestImagePreview(clip.id, maxEdge: 480),
      builder: (context, snapshot) {
        final preview = snapshot.data;
        return Align(
          alignment: Alignment.centerLeft,
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 240, maxHeight: 120),
            child: preview == null
                ? const SizedBox(height: 120, child: Icon(LucideIcons.image))
                : ClipRRect(
                    borderRadius: const BorderRadius.all(
                      Radius.circular(AppRadius.sm),
                    ),
                    child: AspectRatio(
                      aspectRatio: preview.width / preview.height,
                      child: Image.memory(
                        preview.bytes,
                        fit: BoxFit.contain,
                        errorBuilder: (context, error, stackTrace) =>
                            const Icon(LucideIcons.imageOff),
                      ),
                    ),
                  ),
          ),
        );
      },
    );
  }
}

class _Footer extends StatelessWidget {
  const _Footer({required this.controller});

  final QuickPasteController controller;

  @override
  Widget build(BuildContext context) {
    final style = AppTheme.clipboardMenuButtonStyle();
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Button.ghost(
          style: style,
          alignment: Alignment.centerLeft,
          onPressed: () => confirmClear(context, controller),
          child: const Text('Clear'),
        ),
        Button.ghost(
          style: style,
          alignment: Alignment.centerLeft,
          onPressed: controller.openSettings,
          child: Row(
            children: [
              const Expanded(child: Text('Preferences…')),
              MenuShortcut(
                activator: _desktopShortcut(LogicalKeyboardKey.comma),
                combiner: const SizedBox.shrink(),
              ),
            ],
          ),
        ),
        Button.ghost(
          style: style,
          alignment: Alignment.centerLeft,
          onPressed: () => showAbout(context),
          child: const Text('About'),
        ),
        Button.ghost(
          style: style,
          alignment: Alignment.centerLeft,
          onPressed: controller.quit,
          child: Row(
            children: [
              const Expanded(child: Text('Quit')),
              MenuShortcut(
                activator: _desktopShortcut(LogicalKeyboardKey.keyQ),
                combiner: const SizedBox.shrink(),
              ),
            ],
          ),
        ),
      ],
    );
  }

  static Future<void> confirmClear(
    BuildContext context,
    QuickPasteController controller,
  ) {
    return AppOverlays.showDialog<void>(
      context,
      builder: (dialogContext) => AppOverlays.alertDialog(
        icon: LucideIcons.trash2,
        title: const Text('Clear history?'),
        content: const Text('Pinned clips will stay in your history.'),
        actions: [
          Button.secondary(
            onPressed: () => Navigator.pop(dialogContext),
            child: const Text('Cancel'),
          ),
          Button.destructive(
            onPressed: () async {
              final cleared = await controller.clearUnpinned();
              if (cleared && dialogContext.mounted) {
                Navigator.pop(dialogContext);
              }
            },
            child: const Text('Clear'),
          ),
        ],
      ),
    );
  }

  static Future<void> showAbout(BuildContext context) {
    return AppOverlays.showDialog<void>(
      context,
      builder: (dialogContext) => AppOverlays.alertDialog(
        icon: LucideIcons.info,
        title: const Text('CopyPaste'),
        content: const Text(
          'Private clipboard history across macOS, Android, and Windows.',
        ),
        actions: [
          Button.primary(
            onPressed: () => Navigator.pop(dialogContext),
            child: const Text('Done'),
          ),
        ],
      ),
    );
  }
}
