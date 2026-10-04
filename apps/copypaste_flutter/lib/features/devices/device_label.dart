import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../app/theme/app_tokens.dart';
import 'device_presentation.dart';
import 'devices_gateway.dart';

/// The shared `[typed device icon] device name` presentation contract.
class DeviceLabel extends StatelessWidget {
  const DeviceLabel({
    super.key,
    required this.name,
    required this.deviceClass,
    this.iconSize = AppIconSize.sm,
    this.showName = true,
    this.style,
  });

  final String name;
  final DeviceClass deviceClass;
  final double iconSize;
  final bool showName;
  final TextStyle? style;

  @override
  Widget build(BuildContext context) {
    final resolvedStyle = style ?? DefaultTextStyle.of(context).style;
    final leading = SizedBox.square(
      dimension: iconSize,
      child: Icon(
        DevicePresentation.icon(deviceClass),
        size: iconSize,
        color: resolvedStyle.color,
      ),
    );
    if (!showName) return ExcludeSemantics(child: leading);
    return Text.rich(
      TextSpan(
        style: resolvedStyle,
        children: [
          WidgetSpan(
            alignment: PlaceholderAlignment.middle,
            child: ExcludeSemantics(child: leading),
          ),
          const WidgetSpan(child: SizedBox(width: AppSpacing.xs)),
          TextSpan(text: name),
        ],
      ),
      maxLines: 1,
      overflow: TextOverflow.ellipsis,
    );
  }
}
