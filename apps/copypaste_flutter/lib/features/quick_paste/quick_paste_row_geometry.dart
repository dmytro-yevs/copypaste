import 'package:flutter/rendering.dart';
import 'package:flutter/widgets.dart';

/// Reports the scalar geometry of a row without retaining its clip payload.
///
/// The render object captures its own height after it has laid out its child.
/// It posts only that scalar to the widget layer, avoiding reads from a dirty
/// render object during a size-change notification.
class QuickPasteRowGeometry extends SingleChildRenderObjectWidget {
  const QuickPasteRowGeometry({
    super.key,
    required this.onExtent,
    required super.child,
  });

  final ValueChanged<double> onExtent;

  @override
  RenderProxyBox createRenderObject(BuildContext context) =>
      _RenderQuickPasteRowGeometry(onExtent);

  @override
  void updateRenderObject(
    BuildContext context,
    RenderProxyBox renderObject,
  ) {
    (renderObject as _RenderQuickPasteRowGeometry).onExtent = onExtent;
  }
}

class _RenderQuickPasteRowGeometry extends RenderProxyBox {
  _RenderQuickPasteRowGeometry(this._onExtent);

  ValueChanged<double> _onExtent;
  double? _reported;
  double? _pending;
  bool _postFrameScheduled = false;

  set onExtent(ValueChanged<double> value) => _onExtent = value;

  @override
  void performLayout() {
    super.performLayout();
    final extent = size.height;
    if (extent <= 0 || extent == _reported || extent == _pending) return;
    _pending = extent;
    if (_postFrameScheduled) return;
    _postFrameScheduled = true;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _postFrameScheduled = false;
      if (!attached) return;
      final pending = _pending;
      _pending = null;
      if (pending == null || pending == _reported) return;
      _reported = pending;
      _onExtent(pending);
    });
  }
}
