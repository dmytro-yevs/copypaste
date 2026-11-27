import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_toast.dart';
import '../../../app/theme/app_tokens.dart';
import '../controller/history_controller.dart';
import '../repository/history_repository.dart';

/// Lets the user pick paired devices and sends [clipIds] to them now, even
/// while automatic sync is paused.
Future<void> showHistorySendDialog(
  BuildContext context, {
  required HistoryController controller,
  required List<String> clipIds,
}) async {
  final send = controller.capabilities.send;
  if (send == null || clipIds.isEmpty) return;
  final strings = context.l10n;
  final List<HistorySendTarget> targets;
  try {
    targets = await send.sendTargets();
  } catch (_) {
    if (context.mounted) _sendFailed(context, strings.historySendUnavailable);
    return;
  }
  if (!context.mounted) return;
  final selected = ValueNotifier<Set<String>>({
    if (targets.length == 1) targets.single.id,
  });
  List<HistorySendResult>? results;
  try {
    await AppOverlays.showActionDialog(
      context,
      icon: LucideIcons.send,
      title: Text(strings.historySendDialogTitle),
      actionLabel: strings.historySendAction,
      destructive: false,
      changes: selected,
      content: _SendTargets(targets: targets, selected: selected),
      canConfirm: () => selected.value.isNotEmpty,
      onConfirm: () async {
        results = await send.sendClips(clipIds, selected.value.toList());
        return true;
      },
      failureMessage: () => strings.historySendUnavailable,
    );
  } finally {
    selected.dispose();
  }
  final answered = results;
  if (answered == null || !context.mounted) return;
  final failed = answered.where((result) => result.errorCode != null);
  if (failed.isEmpty) {
    AppToast.show(
      context,
      title: strings.historySentTitle,
      message: answered.map((result) => result.name).join(', '),
      tone: AppToastTone.success,
    );
    return;
  }
  final first = failed.first;
  _sendFailed(
    context,
    first.errorCode == 'unsupported_content'
        ? strings.historySendBlocked(first.name)
        : strings.historySendFailed(first.name),
  );
}

void _sendFailed(BuildContext context, String message) => AppToast.show(
  context,
  title: context.l10n.historySendFailedTitle,
  message: message,
  tone: AppToastTone.error,
);

class _SendTargets extends StatelessWidget {
  const _SendTargets({required this.targets, required this.selected});

  final List<HistorySendTarget> targets;
  final ValueNotifier<Set<String>> selected;

  @override
  Widget build(BuildContext context) {
    final strings = context.l10n;
    return ValueListenableBuilder<Set<String>>(
      valueListenable: selected,
      builder: (context, chosen, _) => Column(
        key: const ValueKey<String>('history-send-targets'),
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        spacing: AppSpacing.sm,
        children: [
          Text(
            targets.isEmpty
                ? strings.historySendNoDevices
                : strings.historySendDescription,
          ),
          for (final target in targets)
            Checkbox(
              key: ValueKey<String>('history-send-target-${target.id}'),
              state: chosen.contains(target.id)
                  ? CheckboxState.checked
                  : CheckboxState.unchecked,
              onChanged: (state) => selected.value = {
                ...chosen.where((id) => id != target.id),
                if (state == CheckboxState.checked) target.id,
              },
              trailing: Flexible(child: Text(target.name)),
            ),
        ],
      ),
    );
  }
}
