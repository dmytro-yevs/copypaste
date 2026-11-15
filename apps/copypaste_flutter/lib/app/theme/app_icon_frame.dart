import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'app_tokens.dart';

/// Keeps icon artwork square even when a control stretches its leading slot.
class AppIconFrame extends StatelessWidget {
  const AppIconFrame({super.key, required this.child, this.size});

  final Widget child;
  final double? size;

  @override
  Widget build(BuildContext context) {
    final extent = size ?? IconTheme.of(context).size ?? AppIconSize.sm;
    return SizedBox.square(
      dimension: extent,
      child: Center(
        child: ConstrainedBox(
          constraints: BoxConstraints.loose(Size.square(extent)),
          child: AspectRatio(aspectRatio: 1, child: child),
        ),
      ),
    );
  }
}
