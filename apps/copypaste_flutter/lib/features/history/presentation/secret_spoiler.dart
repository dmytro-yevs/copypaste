import 'dart:math' as math;

import 'package:shadcn_flutter/shadcn_flutter.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../app/theme/app_motion.dart';
import '../../../app/theme/app_theme.dart';

/// A dust spoiler with an explicit reveal and no hidden plaintext semantics.
/// Particle masking and a circular reveal follow Telegram's spoiler behavior;
/// this is an independent Flutter implementation, not a native-code port.
class SecretSpoiler extends StatefulWidget {
  const SecretSpoiler({super.key, required this.reveal});
  final WidgetBuilder reveal;

  @override
  State<SecretSpoiler> createState() => _SecretSpoilerState();
}

class _SecretSpoilerState extends State<SecretSpoiler>
    with TickerProviderStateMixin, WidgetsBindingObserver {
  late final _motion = AppSpoilerMotion(vsync: this);
  bool _revealed = false;
  Offset? _origin;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (!_revealed) {
      _motion.conceal(reducedMotion: AppMotion.reducedMotionOf(context));
    }
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state != AppLifecycleState.resumed && _revealed) {
      setState(() => _revealed = false);
      _motion.conceal(reducedMotion: AppMotion.reducedMotionOf(context));
    }
  }

  void _open() {
    setState(() => _revealed = true);
    _motion.open(reducedMotion: AppMotion.reducedMotionOf(context));
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    _motion.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final color = Theme.of(context).colorScheme.mutedForeground;
    if (_revealed) {
      return AnimatedBuilder(
        animation: _motion.reveal,
        child: widget.reveal(context),
        builder: (context, child) => ClipPath(
          clipper: _RevealClipper(
            AppMotion.spoilerRevealCurve.transform(_motion.reveal.value),
            _origin,
          ),
          child: child,
        ),
      );
    }
    return Semantics(
      label: 'Confidential content. Reveal spoiler',
      child: Listener(
        onPointerDown: (event) => _origin = event.localPosition,
        child: Button.ghost(
          style: AppTheme.controlButtonStyle(const ButtonStyle.ghostIcon()),
          onPressed: _open,
          child: ExcludeSemantics(
            child: SizedBox(
              width: double.infinity,
              height: AppIconSize.sm,
              child: ClipRRect(
                borderRadius: BorderRadius.circular(AppRadius.sm),
                child: CustomPaint(painter: _DustPainter(_motion.dust, color)),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _RevealClipper extends CustomClipper<Path> {
  const _RevealClipper(this.progress, this.origin);
  final double progress;
  final Offset? origin;
  @override
  Path getClip(Size size) {
    if (progress >= 1) return Path()..addRect(Offset.zero & size);
    final center = origin ?? size.center(Offset.zero);
    final radius = math.sqrt(
      size.width * size.width + size.height * size.height,
    );
    return Path()
      ..addOval(Rect.fromCircle(center: center, radius: radius * progress));
  }

  @override
  bool shouldReclip(_RevealClipper old) =>
      old.progress != progress || old.origin != origin;
}

class _DustPainter extends CustomPainter {
  _DustPainter(this.time, this.color) : super(repaint: time);
  final Animation<double> time;
  final Color color;
  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()..color = color.withValues(alpha: 0.12);
    canvas.drawRect(Offset.zero & size, paint);
    // Dust geometry is content-specific; its density is bounded independently of text length.
    final count = (size.width * size.height / 50).round().clamp(24, 256);
    for (var i = 0; i < count; i++) {
      final seed = i * 2.399963;
      final x = ((i * 0.618034 + time.value * 0.08) % 1) * size.width;
      final y =
          ((i * 0.414214 + math.sin(seed + time.value * math.pi * 2) * 0.025) %
              1) *
          size.height;
      paint.color = color.withValues(
        alpha: 0.3 + (math.sin(seed + time.value * math.pi * 2) + 1) * 0.3,
      );
      canvas.drawCircle(Offset(x, y), 0.5 + (i % 3) * 0.25, paint);
    }
  }

  @override
  bool shouldRepaint(_DustPainter old) =>
      old.color != color || old.time != time;
}
