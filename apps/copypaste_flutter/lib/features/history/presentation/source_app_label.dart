import 'package:copypaste_flutter/app/theme/app_tokens.dart';
import 'package:copypaste_flutter/features/history/models/history_models.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

/// The single presentation contract for a clipboard source application.
class SourceAppLabel extends StatelessWidget {
  const SourceAppLabel({
    super.key,
    required this.name,
    required this.icon,
    this.iconSize = AppIconSize.sm,
    this.showName = true,
    this.style,
    this.strutStyle,
  });

  final String name;
  final Future<HistorySourceAppIcon?> icon;
  final double iconSize;
  final bool showName;
  final TextStyle? style;
  final StrutStyle? strutStyle;

  @override
  Widget build(BuildContext context) {
    return FutureBuilder<HistorySourceAppIcon?>(
      future: icon,
      builder: (context, snapshot) {
        final resolvedStyle = style ?? DefaultTextStyle.of(context).style;
        final sourceIcon = snapshot.data;
        final leading = Avatar(
          initials: Avatar.getInitials(name),
          size: iconSize,
          provider: sourceIcon == null ? null : MemoryImage(sourceIcon.bytes),
        );
        if (!showName) return ExcludeSemantics(child: leading);
        return Text.rich(
          TextSpan(
            children: [
              WidgetSpan(
                alignment: PlaceholderAlignment.middle,
                child: ExcludeSemantics(child: leading),
              ),
              const WidgetSpan(child: SizedBox(width: AppSpacing.xs)),
              TextSpan(text: name),
            ],
          ),
          style: resolvedStyle,
          strutStyle: strutStyle,
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
        );
      },
    );
  }
}
