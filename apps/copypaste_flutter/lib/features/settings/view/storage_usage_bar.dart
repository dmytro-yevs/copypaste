import 'package:intl/intl.dart' show NumberFormat;
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_motion.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../../../shared/format_bytes.dart';

/// One part of the space the application uses.
typedef StorageUsagePart = ({String label, int bytes, Color color});

/// The parts of [total] as one segmented bar with a legend under it.
/// Pointing at a segment or its legend entry highlights that part and names
/// its size and share.
class StorageUsageBar extends StatefulWidget {
  const StorageUsageBar({super.key, required this.parts, required this.total});

  final List<StorageUsagePart> parts;
  final int total;

  @override
  State<StorageUsageBar> createState() => _StorageUsageBarState();
}

class _StorageUsageBarState extends State<StorageUsageBar> {
  // Parts not pointed at keep enough color to stay recognizable.
  static const _dimmedOpacity = 0.3;

  int? _hovered;

  void _hover(int? index) {
    if (_hovered != index) setState(() => _hovered = index);
  }

  String _share(BuildContext context, int bytes) {
    final format = NumberFormat.percentPattern(context.l10n.localeName);
    final share = widget.total <= 0 ? 0.0 : bytes / widget.total;
    // A tiny part reads as under one percent rather than zero.
    return share > 0 && share < 0.01
        ? '<${format.format(0.01)}'
        : format.format(share);
  }

  String _detail(BuildContext context, StorageUsagePart part) =>
      '${part.label} · ${formatBytes(part.bytes)} · ${_share(context, part.bytes)}';

  Widget _dimmable(int index, Widget child) => AppMotion.progressTransition(
    context,
    value: _hovered == null || _hovered == index ? 1 : _dimmedOpacity,
    builder: (context, opacity, _) => Opacity(opacity: opacity, child: child),
  );

  @override
  Widget build(BuildContext context) {
    final total = widget.total;
    // Every non-empty part stays visible, however small its share.
    int flex(int bytes) =>
        total <= 0 ? 0 : (bytes * 1000 ~/ total).clamp(1, 1000);
    final visible = [
      for (final (index, part) in widget.parts.indexed)
        if (part.bytes > 0) (index, part),
    ];
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        ClipRRect(
          borderRadius: const BorderRadius.all(Radius.circular(AppRadius.sm)),
          child: SizedBox(
            height: AppSpacing.lg,
            child: visible.isEmpty
                ? ColoredBox(color: Theme.of(context).colorScheme.muted)
                : Row(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    spacing: AppSpacing.xxs,
                    children: [
                      for (final (index, part) in visible)
                        Expanded(
                          flex: flex(part.bytes),
                          child: MouseRegion(
                            onEnter: (_) => _hover(index),
                            onExit: (_) => _hover(null),
                            child: Tooltip(
                              tooltip: (context) => TooltipContainer(
                                child: Text(_detail(context, part)),
                              ),
                              child: Semantics(
                                label: _detail(context, part),
                                child: _dimmable(
                                  index,
                                  ColoredBox(color: part.color),
                                ),
                              ),
                            ),
                          ),
                        ),
                    ],
                  ),
          ),
        ),
        const Gap(AppSpacing.md),
        Wrap(
          spacing: AppSpacing.lg,
          runSpacing: AppSpacing.sm,
          children: [
            for (final (index, part) in widget.parts.indexed)
              MouseRegion(
                onEnter: (_) => _hover(index),
                onExit: (_) => _hover(null),
                child: _dimmable(
                  index,
                  Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      SizedBox.square(
                        dimension: AppIconSize.statusIndicator,
                        child: DecoratedBox(
                          decoration: BoxDecoration(
                            color: part.color,
                            shape: BoxShape.circle,
                          ),
                        ),
                      ),
                      const Gap(AppSpacing.xs),
                      Text(part.label).textSmall(),
                      const Gap(AppSpacing.xs),
                      Text(
                        _hovered == index
                            ? '${formatBytes(part.bytes)} · ${_share(context, part.bytes)}'
                            : formatBytes(part.bytes),
                      ).textSmall().muted(),
                    ],
                  ),
                ),
              ),
          ],
        ),
      ],
    );
  }
}
