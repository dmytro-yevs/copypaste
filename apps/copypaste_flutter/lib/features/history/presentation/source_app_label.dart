import 'package:copypaste_flutter/app/theme/app_tokens.dart';
import 'package:copypaste_flutter/app/theme/app_icon_frame.dart';
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

  /// Decodes the icon for the physical size of the square avatar. The avatar
  /// covers its frame, so a known non-square icon scales by its shorter side.
  static ImageProvider iconImage(
    BuildContext context,
    HistorySourceAppIcon icon, {
    double size = AppIconSize.sm,
  }) {
    final edge = (size * MediaQuery.devicePixelRatioOf(context)).ceil();
    final source = MemoryImage(icon.bytes);
    final width = icon.width;
    final height = icon.height;
    if (width != null && height != null && width != height) {
      return width < height
          ? ResizeImage(source, width: edge)
          : ResizeImage(source, height: edge);
    }
    return ResizeImage(
      source,
      width: edge,
      height: edge,
      policy: ResizeImagePolicy.fit,
    );
  }

  @override
  Widget build(BuildContext context) {
    return FutureBuilder<HistorySourceAppIcon?>(
      future: icon,
      builder: (context, snapshot) {
        final resolvedStyle = style ?? DefaultTextStyle.of(context).style;
        final sourceIcon = snapshot.data;
        final leading = AppIconFrame(
          size: iconSize,
          child: Avatar(
            initials: Avatar.getInitials(name),
            size: iconSize,
            provider: sourceIcon == null
                ? null
                : iconImage(context, sourceIcon, size: iconSize),
          ),
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
