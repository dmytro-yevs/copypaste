import 'package:shadcn_flutter/shadcn_flutter.dart';
import '../../../app/theme/app_tokens.dart';

class HistoryColorSwatch extends StatelessWidget {
  const HistoryColorSwatch({super.key, required this.rgba, this.size = 32});

  final int rgba;
  final double size;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Container(
      width: size,
      height: size,
      padding: const EdgeInsets.all(AppSpacing.xxs),
      decoration: BoxDecoration(
        color: scheme.muted,
        border: Border.all(color: scheme.border),
        borderRadius: BorderRadius.circular(AppRadius.sm),
      ),
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: Color.fromARGB(
            rgba & 0xff,
            (rgba >> 24) & 0xff,
            (rgba >> 16) & 0xff,
            (rgba >> 8) & 0xff,
          ),
          borderRadius: BorderRadius.circular(AppRadius.xs),
        ),
      ),
    );
  }
}
