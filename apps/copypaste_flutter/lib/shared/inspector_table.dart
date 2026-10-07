import 'dart:math' as math;

import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../app/theme/app_theme.dart';
import '../app/theme/app_tokens.dart';

/// Metadata for an inspector, with selectable values and optional identity UI.
typedef InspectorTableRow = ({String label, Widget value});

/// Shared inspector layout using the stock shadcn table and row borders.
class InspectorTable extends StatelessWidget {
  const InspectorTable({
    super.key,
    required this.rows,
    this.tableKey,
    this.compact = false,
  });

  final List<InspectorTableRow> rows;
  final Key? tableKey;
  final bool compact;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final style = AppTheme.inspectorTextStyle(context, compact: compact);
    return LayoutBuilder(
      builder: (context, constraints) => Table(
        key: tableKey,
        columnWidths: {
          0: FixedTableSize(
            math.min(
              constraints.maxWidth * AppLayoutSize.inspectorLabelWidthFactor,
              AppLayoutSize.inspectorLabelMaxWidth,
            ),
          ),
          1: const FlexTableSize(),
        },
        rows: [
          for (final row in rows)
            TableRow(
              cells: [
                TableCell(
                  child: _cell(
                    Text(
                      row.label,
                      style: style
                          .merge(theme.typography.medium)
                          .copyWith(color: theme.colorScheme.mutedForeground),
                    ),
                  ),
                ),
                TableCell(
                  child: _cell(
                    DefaultTextStyle.merge(style: style, child: row.value),
                  ),
                ),
              ],
            ),
        ],
      ),
    );
  }

  Widget _cell(Widget child) => Padding(
    padding: AppTheme.inspectorCellPadding,
    child: Align(alignment: AppTheme.inspectorCellAlignment, child: child),
  );
}
