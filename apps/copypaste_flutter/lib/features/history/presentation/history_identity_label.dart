import 'package:copypaste_flutter/app/theme/app_tokens.dart';
import 'package:copypaste_flutter/features/devices/device_presentation.dart';
import 'package:copypaste_flutter/features/devices/devices_gateway.dart';
import 'package:copypaste_flutter/features/history/models/history_models.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

/// The single presentation contract for application and device identities.
///
/// Application icons are resolved by the caller's history controller so data
/// access and caching remain outside the widget. Device labels reuse the typed
/// device presentation used by the Devices screen.
class HistoryIdentityLabel extends StatelessWidget {
  const HistoryIdentityLabel.application({
    super.key,
    required this.name,
    required Future<HistorySourceAppIcon?> icon,
    this.iconSize = AppIconSize.sm,
    this.style,
  }) : _sourceIcon = icon,
       _deviceClass = null;

  const HistoryIdentityLabel.device({
    super.key,
    required this.name,
    required DeviceClass deviceClass,
    this.iconSize = AppIconSize.sm,
    this.style,
  }) : _sourceIcon = null,
       _deviceClass = deviceClass;

  final String name;
  final Future<HistorySourceAppIcon?>? _sourceIcon;
  final DeviceClass? _deviceClass;
  final double iconSize;
  final TextStyle? style;

  @override
  Widget build(BuildContext context) {
    final sourceIcon = _sourceIcon;
    if (sourceIcon == null) {
      return _label(context, null);
    }
    return FutureBuilder<HistorySourceAppIcon?>(
      future: sourceIcon,
      builder: (context, snapshot) => _label(context, snapshot.data),
    );
  }

  Widget _label(BuildContext context, HistorySourceAppIcon? sourceIcon) {
    final resolvedStyle = style ?? DefaultTextStyle.of(context).style;
    final deviceClass = _deviceClass;
    final leading = deviceClass == null
        ? Avatar(
            initials: Avatar.getInitials(name),
            size: iconSize,
            provider: sourceIcon == null ? null : MemoryImage(sourceIcon.bytes),
          )
        : SizedBox.square(
            dimension: iconSize,
            child: Icon(
              DevicePresentation.icon(deviceClass),
              size: iconSize,
              color: resolvedStyle.color,
            ),
          );
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
