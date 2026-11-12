import 'package:flutter/foundation.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/shell/app_header.dart';
import '../../../app/shell/header_action_content.dart';
import '../../../app/shell/header_fit.dart';
import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_motion.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../models/settings_models.dart';
import '../models/storage_usage_text.dart';

/// Storage status cell: a meter of the quota in use with the figures while
/// the header has room. It is left out until the runtime reports usage.
class StorageHeaderAction extends StatelessWidget
    implements OptionalHeaderStatus {
  const StorageHeaderAction({
    super.key,
    required this.usage,
    required this.onPressed,
  });

  final ValueListenable<StorageUsage?> usage;
  final VoidCallback onPressed;

  @override
  Listenable get presence => usage;

  @override
  bool get present => usage.value != null;

  /// Settings shows the same usage; the bottom-navigation header keeps its
  /// room for the destination name.
  @override
  HeaderStage? get until => HeaderStage.bottomNavigation;

  @override
  Widget build(BuildContext context) {
    final stage = HeaderFit.stageOf(context);
    // Left out from [until] on, the cell is only measured, so it does not
    // keep the readout observed.
    if (stage.index >= until!.index) return _cell(context, usage.value);
    return ValueListenableBuilder<StorageUsage?>(
      valueListenable: usage,
      builder: (context, usage, _) => _cell(context, usage),
    );
  }

  Widget _cell(BuildContext context, StorageUsage? usage) {
    final worded = HeaderFit.stageOf(context).showsStatusText;
    if (usage == null) return const SizedBox.shrink();
    final l10n = context.l10n;
    final summary = StorageUsageText.summary(l10n, usage);
    final short = StorageUsageText.short(l10n, usage);
    final over = usage.overQuota;
    final content = Semantics(
      label: l10n.storageHeaderSemanticsLabel(summary),
      child: ExcludeSemantics(
        child: HeaderActionContent(
          // Without the words the control itself reports the overrun.
          icon: AppIconFrame(
            child: over && !worded
                ? Icon(
                    LucideIcons.circleAlert,
                    color: AppTheme.statusColor(context, AppStatusTone.warning),
                  )
                : _StorageMeter(usage: usage),
          ),
          label: over ? l10n.storageUsageOverQuota : null,
          detail: over ? ' · $short' : short,
        ),
      ),
    );
    return Tooltip(
      showDuration: AppMotion.resolve(context, AppMotion.standard),
      tooltip: (context) => TooltipContainer(child: Text(summary)),
      child: Button.ghost(
        key: const ValueKey<String>('storage-header-open'),
        style: AppTheme.headerStatusCellStyle,
        onPressed: onPressed,
        child: content,
      ),
    );
  }
}

/// A static ring filled to the used share of the quota, or a drive glyph
/// when no quota applies.
class _StorageMeter extends StatelessWidget {
  const _StorageMeter({required this.usage});

  final StorageUsage usage;

  @override
  Widget build(BuildContext context) {
    final fraction = usage.fraction;
    if (fraction == null) return const Icon(LucideIcons.hardDrive);
    return CircularProgressIndicator(
      key: const ValueKey<String>('storage-header-meter'),
      value: fraction,
      animated: false,
      size: IconTheme.of(context).size ?? AppIconSize.sm,
      strokeWidth: AppSpacing.xxs,
      color: usage.overQuota
          ? AppTheme.statusColor(context, AppStatusTone.warning)
          : Theme.of(context).colorScheme.foreground,
    );
  }
}
