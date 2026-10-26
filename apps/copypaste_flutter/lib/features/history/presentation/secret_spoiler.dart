import 'dart:async';
import 'dart:math' as math;

import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_tokens.dart';
import '../../../app/theme/app_motion.dart';
import '../../../app/theme/app_theme.dart';
import '../controller/history_spoiler_controller.dart';

/// Uses the content's actual layout while excluding its paint, focus, pointer
/// targets and semantics until the shared presentation state reveals the clip.
class SecretSpoiler extends StatefulWidget {
  const SecretSpoiler({
    super.key,
    required this.clipId,
    required this.controller,
    required this.child,
  });
  final String clipId;
  final HistorySpoilerController controller;
  final Widget child;

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
    widget.controller.addListener(_changed);
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _updateMotion();
  }

  @override
  void didUpdateWidget(SecretSpoiler oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.controller != widget.controller) {
      oldWidget.controller.removeListener(_changed);
      widget.controller.addListener(_changed);
    }
    _updateMotion();
  }

  void _changed() {
    if (!mounted) return;
    setState(_updateMotion);
  }

  void _updateMotion() {
    final revealed = widget.controller.isRevealed(widget.clipId);
    if (revealed) {
      if (!_revealed) {
        _motion.open(reducedMotion: AppMotion.reducedMotionOf(context));
      }
    } else {
      _motion.conceal(reducedMotion: AppMotion.reducedMotionOf(context));
    }
    _revealed = revealed;
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if ((state == AppLifecycleState.hidden ||
            state == AppLifecycleState.paused ||
            state == AppLifecycleState.detached) &&
        widget.controller.isRevealed(widget.clipId)) {
      unawaited(widget.controller.setRevealed(null, false));
    }
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    widget.controller.removeListener(_changed);
    _motion.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final color = Theme.of(context).colorScheme.mutedForeground;
    return Stack(
      children: [
        Visibility(
          visible: _revealed,
          maintainState: true,
          maintainAnimation: true,
          maintainSize: true,
          child: AnimatedBuilder(
            animation: _motion.reveal,
            child: widget.child,
            builder: (context, child) => ClipPath(
              clipper: _RevealClipper(
                AppMotion.spoilerRevealCurve.transform(_motion.reveal.value),
                _origin,
              ),
              child: child,
            ),
          ),
        ),
        if (!_revealed)
          Positioned.fill(
            child: Semantics(
              label: 'Confidential content. Reveal spoiler',
              child: Listener(
                onPointerDown: (event) => _origin = event.localPosition,
                child: Button.ghost(
                  style: AppTheme.spoilerButtonStyle,
                  onPressed: () =>
                      widget.controller.setRevealed(widget.clipId, true),
                  child: ExcludeSemantics(
                    child: SizedBox.expand(
                      child: ClipRRect(
                        borderRadius: BorderRadius.circular(AppRadius.sm),
                        child: CustomPaint(
                          painter: _DustPainter(_motion.dust, color),
                        ),
                      ),
                    ),
                  ),
                ),
              ),
            ),
          ),
      ],
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
