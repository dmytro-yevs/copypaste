import 'package:shadcn_flutter/shadcn_flutter.dart';

/// The CopyPaste brand mark, decoded at its displayed physical size rather
/// than at the resolution of the launcher-icon source asset.
class AppBrandLogo extends StatelessWidget {
  const AppBrandLogo({super.key, required this.size, this.semanticLabel});

  static const asset = 'assets/brand/copypaste.png';

  final double size;
  final String? semanticLabel;

  @override
  Widget build(BuildContext context) {
    final edge = (size * MediaQuery.devicePixelRatioOf(context)).ceil();
    return Image(
      image: ResizeImage(
        const AssetImage(asset),
        width: edge,
        height: edge,
        policy: ResizeImagePolicy.fit,
      ),
      width: size,
      height: size,
      fit: BoxFit.contain,
      semanticLabel: semanticLabel,
    );
  }
}
