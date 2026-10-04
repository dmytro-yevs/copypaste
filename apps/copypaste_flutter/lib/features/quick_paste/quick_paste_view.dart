import 'dart:async';
import 'dart:io';

import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../app/theme/app_motion.dart';
import '../../app/theme/app_overlays.dart';
import '../../app/theme/app_tokens.dart';
import '../../shared/state_view.dart';
import '../history/controller/history_controller.dart';
import '../history/models/history_models.dart';
import '../history/presentation/history_identity_label.dart';
import 'quick_paste_controller.dart';

class QuickPasteView extends StatefulWidget {
  const QuickPasteView({super.key, required this.controller});

  final QuickPasteController controller;

  @override
  State<QuickPasteView> createState() => _QuickPasteViewState();
}

class _QuickPasteViewState extends State<QuickPasteView> {
  QuickPasteController get controller => widget.controller;

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: controller,
      builder: (context, child) {
        return CallbackShortcuts(
          bindings: _shortcutBindings(context),
          child: Focus(
            autofocus: true,
            child: Scaffold(
              child: Padding(
                padding: const EdgeInsets.all(AppSpacing.sm),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    _Header(controller: controller),
                    const Gap(AppSpacing.sm),
                    Expanded(
                      child: Command(
                        key: ValueKey<int>(controller.presentationGeneration),
                        autofocus: true,
                        debounceDuration: const Duration(milliseconds: 120),
                        searchPlaceholder: const Text('Type to search…'),
                        builder: _buildCommands,
                        loadingBuilder: (context) => const StateView.loading(
                          message: 'Loading clipboard history.',
                        ),
                        emptyBuilder: (context) => const StateView.empty(
                          title: 'No clips found',
                          message: 'Try a different search.',
                        ),
                        errorBuilder: (context, error, stackTrace) =>
                            StateView.error(
                              title: 'History is unavailable',
                              message: controller.history.errorMessage,
                            ),
                      ),
                    ),
                    if (Platform.isMacOS &&
                        controller.autoPaste &&
                        !controller.accessibilityGranted) ...[
                      const Gap(AppSpacing.sm),
                      const Alert(
                        leading: Icon(LucideIcons.accessibility),
                        title: Text('Auto-paste needs Accessibility'),
                        content: Text(
                          'Selection will copy until permission is enabled in System Settings.',
                        ),
                      ),
                    ],
                    const Gap(AppSpacing.sm),
                    const Divider(),
                    const Gap(AppSpacing.sm),
                    _Footer(controller: controller),
                  ],
                ),
              ),
            ),
          ),
        );
      },
    );
  }

  Map<ShortcutActivator, VoidCallback> _shortcutBindings(BuildContext context) {
    final bindings = <ShortcutActivator, VoidCallback>{
      const SingleActivator(LogicalKeyboardKey.escape): () {
        unawaited(controller.close());
      },
      const SingleActivator(LogicalKeyboardKey.enter, shift: true): () {
        unawaited(controller.activateFocused(plainText: true));
      },
      const SingleActivator(LogicalKeyboardKey.enter, alt: true): () {
        unawaited(controller.activateFocused(invertAutoPaste: true));
      },
      const SingleActivator(
        LogicalKeyboardKey.enter,
        alt: true,
        shift: true,
      ): () {
        unawaited(
          controller.activateFocused(plainText: true, forcePaste: true),
        );
      },
      const SingleActivator(LogicalKeyboardKey.keyP, alt: true): () {
        unawaited(controller.toggleFocusedPin());
      },
      const SingleActivator(LogicalKeyboardKey.backspace, alt: true): () {
        unawaited(controller.deleteFocused());
      },
      const SingleActivator(LogicalKeyboardKey.delete, alt: true): () {
        unawaited(controller.deleteFocused());
      },
      const SingleActivator(LogicalKeyboardKey.pageDown): () {
        _moveFocus(context, forward: true);
      },
      const SingleActivator(LogicalKeyboardKey.pageUp): () {
        _moveFocus(context, forward: false);
      },
    };
    final digits = <LogicalKeyboardKey>[
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
    for (var index = 0; index < digits.length; index += 1) {
      bindings[SingleActivator(
        digits[index],
        meta: Platform.isMacOS,
        control: Platform.isWindows,
      )] = () =>
          unawaited(controller.activateIndex(index));
    }
    return bindings;
  }

  void _moveFocus(BuildContext context, {required bool forward}) {
    final scope = FocusScope.of(context);
    for (var step = 0; step < 5; step += 1) {
      if (forward) {
        scope.nextFocus();
      } else {
        scope.previousFocus();
      }
    }
  }

  Stream<List<Widget>> _buildCommands(
    BuildContext context,
    String? query,
  ) async* {
    await controller.search(query ?? '');
    if (controller.history.state == HistoryLoadState.error) {
      throw StateError(
        controller.history.errorMessage ?? 'Clipboard history is unavailable.',
      );
    }
    final indexed = controller.history.items.indexed.toList(growable: false);
    final pinned = indexed.where((entry) => entry.$2.pinned).toList();
    final recent = indexed.where((entry) => !entry.$2.pinned).toList();
    yield [
      if (pinned.isNotEmpty)
        CommandCategory(
          title: const Text('Pinned'),
          children: [for (final entry in pinned) _item(entry.$2, entry.$1)],
        ),
      if (recent.isNotEmpty)
        CommandCategory(
          title: pinned.isEmpty ? null : const Text('History'),
          children: [for (final entry in recent) _item(entry.$2, entry.$1)],
        ),
    ];
  }

  Widget _item(HistoryClip clip, int index) {
    final shortcut = index < 9
        ? SingleActivator(
            <LogicalKeyboardKey>[
              LogicalKeyboardKey.digit1,
              LogicalKeyboardKey.digit2,
              LogicalKeyboardKey.digit3,
              LogicalKeyboardKey.digit4,
              LogicalKeyboardKey.digit5,
              LogicalKeyboardKey.digit6,
              LogicalKeyboardKey.digit7,
              LogicalKeyboardKey.digit8,
              LogicalKeyboardKey.digit9,
            ][index],
            meta: Platform.isMacOS,
            control: Platform.isWindows,
          )
        : null;
    return Focus(
      onFocusChange: (focused) {
        if (focused) controller.focus(clip);
      },
      child: Tooltip(
        showDuration: AppMotion.resolve(context, AppMotion.standard),
        tooltip: (context) => TooltipContainer(
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 460),
            child: Text(
              clip.preview.isEmpty ? clip.contentKind.label : clip.preview,
            ),
          ),
        ),
        child: CommandItem(
          key: ValueKey<String>('quick-paste-${clip.id}'),
          leading: _ClipLeading(clip: clip, controller: controller),
          title: _ClipTitle(clip: clip, controller: controller),
          trailing: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              if (clip.pinned)
                const Icon(LucideIcons.pin, size: AppIconSize.sm),
              if (shortcut != null) ...[
                if (clip.pinned) const Gap(AppSpacing.sm),
                KeyboardDisplay.fromActivator(activator: shortcut),
              ],
            ],
          ),
          onTap: controller.activating
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
        ),
      ),
    );
  }
}

class _Header extends StatelessWidget {
  const _Header({required this.controller});

  final QuickPasteController controller;

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        const Image(
          image: AssetImage('assets/brand/copypaste.png'),
          width: 24,
          height: 24,
        ),
        const Gap(AppSpacing.sm),
        Text('CopyPaste', style: Theme.of(context).typography.h4),
        const Spacer(),
        Tooltip(
          showDuration: AppMotion.resolve(context, AppMotion.standard),
          tooltip: (context) => TooltipContainer(
            child: Text(
              controller.focusedClip?.pinned == true
                  ? 'Unpin clip'
                  : 'Pin clip',
            ),
          ),
          child: Button.ghost(
            style: const ButtonStyle.ghostIcon(),
            onPressed: controller.focusedClip == null || controller.activating
                ? null
                : controller.toggleFocusedPin,
            child: Icon(
              controller.focusedClip?.pinned == true
                  ? LucideIcons.pinOff
                  : LucideIcons.pin,
            ),
          ),
        ),
        Tooltip(
          showDuration: AppMotion.resolve(context, AppMotion.standard),
          tooltip: (context) =>
              const TooltipContainer(child: Text('Delete clip')),
          child: Button.ghost(
            style: const ButtonStyle.ghostIcon(),
            onPressed: controller.focusedClip == null || controller.activating
                ? null
                : controller.deleteFocused,
            child: const Icon(LucideIcons.trash2),
          ),
        ),
        Tooltip(
          showDuration: AppMotion.resolve(context, AppMotion.standard),
          tooltip: (context) => const TooltipContainer(child: Text('Close')),
          child: Button.ghost(
            style: const ButtonStyle.ghostIcon(),
            onPressed: controller.close,
            child: const Icon(LucideIcons.x),
          ),
        ),
      ],
    );
  }
}

class _ClipTitle extends StatelessWidget {
  const _ClipTitle({required this.clip, required this.controller});

  final HistoryClip clip;
  final QuickPasteController controller;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          clip.preview.isEmpty ? clip.contentKind.label : clip.preview,
          maxLines: clip.contentKind == HistoryClipKind.image ? 1 : 2,
          overflow: TextOverflow.ellipsis,
        ),
        if (clip.sourceApp case final sourceApp?) ...[
          const Gap(AppSpacing.xxs),
          HistoryIdentityLabel.application(
            name: sourceApp,
            icon: controller.history.requestSourceIcon(clip.id),
            style: theme.typography.xSmall.copyWith(
              color: theme.colorScheme.mutedForeground,
            ),
          ),
        ],
      ],
    );
  }
}

class _ClipLeading extends StatelessWidget {
  const _ClipLeading({required this.clip, required this.controller});

  final HistoryClip clip;
  final QuickPasteController controller;

  @override
  Widget build(BuildContext context) {
    if (clip.contentKind == HistoryClipKind.image) {
      return FutureBuilder<HistoryImagePreview?>(
        future: controller.history.requestImagePreview(clip.id, maxEdge: 96),
        builder: (context, snapshot) {
          final preview = snapshot.data;
          return SizedBox(
            width: 56,
            height: 44,
            child: preview == null
                ? const Icon(LucideIcons.image)
                : ClipRRect(
                    borderRadius: BorderRadius.circular(AppRadius.sm),
                    child: Image.memory(
                      preview.bytes,
                      fit: BoxFit.contain,
                      errorBuilder: (context, error, stackTrace) =>
                          const Icon(LucideIcons.imageOff),
                    ),
                  ),
          );
        },
      );
    }
    return Icon(_kindIcon(clip.contentKind), size: AppIconSize.md);
  }
}

class _Footer extends StatelessWidget {
  const _Footer({required this.controller});

  final QuickPasteController controller;

  @override
  Widget build(BuildContext context) {
    return Wrap(
      alignment: WrapAlignment.spaceBetween,
      runSpacing: AppSpacing.xs,
      children: [
        Button.ghost(
          onPressed: controller.openMainWindow,
          child: const Text('Open CopyPaste'),
        ),
        Button.ghost(
          onPressed: () => _confirmClear(context),
          child: const Text('Clear unpinned'),
        ),
        Button.ghost(
          onPressed: controller.openSettings,
          child: const Text('Settings'),
        ),
        Button.ghost(
          onPressed: () => _showAbout(context),
          child: const Text('About'),
        ),
        Button.ghost(onPressed: controller.quit, child: const Text('Quit')),
      ],
    );
  }

  Future<void> _confirmClear(BuildContext context) {
    return AppOverlays.showDialog<void>(
      context,
      builder: (dialogContext) => AppOverlays.alertDialog(
        icon: LucideIcons.trash2,
        title: const Text('Clear unpinned clips?'),
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

  Future<void> _showAbout(BuildContext context) {
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

IconData _kindIcon(HistoryClipKind kind) => switch (kind) {
  HistoryClipKind.text => LucideIcons.type,
  HistoryClipKind.link => LucideIcons.link,
  HistoryClipKind.email => LucideIcons.mail,
  HistoryClipKind.color => LucideIcons.palette,
  HistoryClipKind.phone => LucideIcons.phone,
  HistoryClipKind.code => LucideIcons.code,
  HistoryClipKind.json => LucideIcons.braces,
  HistoryClipKind.path => LucideIcons.folder,
  HistoryClipKind.image => LucideIcons.image,
  HistoryClipKind.file => LucideIcons.file,
  HistoryClipKind.other => LucideIcons.clipboard,
};
