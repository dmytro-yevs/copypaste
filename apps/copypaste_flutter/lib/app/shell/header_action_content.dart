import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../theme/app_theme.dart';
import '../theme/app_tokens.dart';
import 'header_fit.dart';

/// Icon-led content of a header control that can also carry text. The text
/// leaves in header stages while the control keeps the shared height.
class HeaderActionContent extends StatelessWidget {
  const HeaderActionContent({
    super.key,
    required this.icon,
    this.label,
    this.labelUntil = HeaderStage.compact,
    this.detail,
  });

  final Widget icon;
  final String? label;

  /// The first stage that hides [label].
  final HeaderStage labelUntil;

  /// Secondary status text, which leaves at [HeaderStage.labeled]; a status
  /// cell's icon stays in every stage.
  final String? detail;

  @override
  Widget build(BuildContext context) => SizedBox(
    height: AppTheme.controlHeight(context),
    child: Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        icon,
        if (label case final label?)
          HeaderCollapsible(
            until: labelUntil,
            child: Padding(
              padding: const EdgeInsetsDirectional.only(start: AppSpacing.sm),
              child: Text(label, maxLines: 1),
            ),
          ),
        if (detail case final detail?)
          HeaderCollapsible(
            until: HeaderStage.labeled,
            child: Padding(
              padding: EdgeInsetsDirectional.only(
                start: label == null ? AppSpacing.sm : 0,
              ),
              child: Text(detail, maxLines: 1).muted(),
            ),
          ),
      ],
    ),
  );
}
