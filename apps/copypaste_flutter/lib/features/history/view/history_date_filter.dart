import 'dart:async';

import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_filter_indicator.dart';
import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';
import '../controller/history_controller.dart';
import '../../../shared/system_date_time.dart';

/// Date filtering uses the library's calendar and the shared History query.
class HistoryDateFilter extends StatelessWidget {
  const HistoryDateFilter({super.key, required this.controller});
  final HistoryController controller;

  @override
  Widget build(BuildContext context) {
    final query = controller.query;
    final from = query.capturedFrom;
    final before = query.capturedBefore;
    final end = before == null
        ? null
        : DateTime(before.year, before.month, before.day - 1);
    final label = from == null || end == null
        ? 'Filter by date'
        : formatSystemDateRange(context, from, end);
    return AppFilterIndicator(
      active: query.hasDateFilter,
      child: OverlayAnchor(
        anchor: ('history-date-filter', controller),
        child: Tooltip(
          tooltip: (_) => TooltipContainer(child: Text(label)),
          child: Semantics(
            label: label,
            toggled: query.hasDateFilter,
            button: true,
            child: Button.secondary(
              key: const ValueKey('history-date-filter'),
              style: AppTheme.historyToolbarIconStyle,
              onPressed: () => _show(context, from, end),
              child: AppIconFrame(
                size: AppIconSize.md,
                child: Icon(
                  query.hasDateFilter
                      ? LucideIcons.calendarCheck
                      : LucideIcons.calendarRange,
                  size: AppIconSize.md,
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }

  Future<void> _show(
    BuildContext context,
    DateTime? start,
    DateTime? end,
  ) async {
    await AppOverlays.showCompactEditor<void>(
      context,
      anchor: ('history-date-filter', controller),
      builder: (context) =>
          _DateEditor(controller: controller, start: start, end: end),
    );
  }
}

class _DateEditor extends StatefulWidget {
  const _DateEditor({required this.controller, this.start, this.end});
  final HistoryController controller;
  final DateTime? start;
  final DateTime? end;
  @override
  State<_DateEditor> createState() => _DateEditorState();
}

class _DateEditorState extends State<_DateEditor> {
  late CalendarValue? _value = widget.start == null || widget.end == null
      ? null
      : CalendarValue.range(widget.start!, widget.end!);

  Future<void> _preset(int? days, {bool yesterday = false}) async {
    final pending = widget.controller.setDatePreset(days, yesterday: yesterday);
    await closeOverlay(context);
    await pending;
  }

  @override
  Widget build(BuildContext context) {
    final compact =
        OverlayConfiguration.maybeOf(context) is DrawerConfiguration;
    final presets = [
      Button.outline(onPressed: () => _preset(1), child: const Text('Today')),
      Button.outline(
        onPressed: () => _preset(1, yesterday: true),
        child: const Text('Yesterday'),
      ),
      Button.outline(
        onPressed: () => _preset(7),
        child: const Text('Last 7 days'),
      ),
      Button.outline(
        onPressed: () => _preset(30),
        child: const Text('Last 30 days'),
      ),
      Button.outline(
        onPressed: () => _preset(null),
        child: const Text('All time'),
      ),
    ];
    final calendar = Theme(
      data: AppTheme.calendarTheme(context),
      child: DatePickerDialog(
        initialView: widget.start == null
            ? null
            : CalendarView(widget.start!.year, widget.start!.month),
        initialViewType: CalendarViewType.date,
        selectionMode: CalendarSelectionMode.range,
        viewMode: CalendarSelectionMode.single,
        initialValue: _value,
        onChanged: (value) => setState(() => _value = value),
      ),
    );
    final body = compact
        ? FittedBox(
            fit: BoxFit.contain,
            alignment: Alignment.topCenter,
            child: calendar,
          )
        : SingleChildScrollView(
            child: Row(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                IntrinsicWidth(
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    spacing: AppSpacing.sm,
                    children: presets,
                  ),
                ),
                const Gap(AppSpacing.md),
                calendar,
              ],
            ),
          );
    final editor = Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (compact) ...[
          Wrap(
            spacing: AppSpacing.sm,
            runSpacing: AppSpacing.sm,
            children: presets,
          ),
          const Gap(AppSpacing.md),
        ],
        Flexible(child: body),
        const Gap(AppSpacing.md),
        const Divider(),
        const Gap(AppSpacing.md),
        Row(
          mainAxisAlignment: MainAxisAlignment.end,
          spacing: AppSpacing.sm,
          children: [
            Button.outline(
              onPressed: () => closeOverlay(context),
              child: const Text('Cancel'),
            ),
            Button.primary(
              onPressed: _value == null
                  ? null
                  : () async {
                      final range = _value!.toRange();
                      final pending = widget.controller.setDateRange(
                        range.start,
                        range.end,
                      );
                      await closeOverlay(context);
                      await pending;
                    },
              child: const Text('Apply'),
            ),
          ],
        ),
      ],
    );
    return compact ? editor : IntrinsicWidth(child: editor);
  }
}
