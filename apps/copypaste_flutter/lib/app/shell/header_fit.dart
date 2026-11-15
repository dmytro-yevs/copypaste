import 'package:flutter/rendering.dart';
import 'package:flutter/widgets.dart';

/// What the application header gives up, in order, as its content stops
/// fitting the window.
enum HeaderStage {
  /// Navigation, wordmark, labelled destination commands and worded status
  /// with its secondary text.
  detailed,

  /// Destination commands give way first and keep only their icons.
  iconCommands,

  /// Status keeps its state words and drops the secondary text: the sync
  /// device count and, once the status carries it, the storage figures.
  labeled,

  /// Status falls back to icon-only controls.
  compact,

  /// The wordmark leaves its room to the destinations.
  unbranded,

  /// The destinations move to the bottom navigation.
  bottomNavigation;

  bool get showsStatusText => index < compact.index;

  bool get showsHeaderNavigation => this != bottomNavigation;
}

/// Picks the first [HeaderStage] whose header content fits the window and
/// rebuilds its subtree with it during layout, so a width never shows a stage
/// that overflows and never oscillates between two.
class HeaderFit extends AbstractLayoutBuilder<HeaderStage> {
  const HeaderFit({
    super.key,
    required this.minimumWidth,
    this.horizontalInset = 0,
    required this.stageBuilder,
  });

  /// Below this window width the destinations always use bottom navigation.
  final double minimumWidth;

  /// Window width the header cannot use, such as horizontal safe areas.
  final double horizontalInset;

  final Widget Function(BuildContext context, HeaderStage stage) stageBuilder;

  /// Controls outside an adaptive header keep their most compact form.
  static HeaderStage stageOf(BuildContext context) =>
      context.dependOnInheritedWidgetOfExactType<_HeaderStageScope>()?.stage ??
      HeaderStage.bottomNavigation;

  @override
  Widget Function(BuildContext context, HeaderStage stage) get builder =>
      (context, stage) =>
          _HeaderStageScope(stage: stage, child: stageBuilder(context, stage));

  @override
  RenderAbstractLayoutBuilderMixin<HeaderStage, RenderObject>
  createRenderObject(BuildContext context) => _RenderHeaderFit(
    minimumWidth: minimumWidth,
    horizontalInset: horizontalInset,
  );

  @override
  void updateRenderObject(
    BuildContext context,
    covariant RenderObject renderObject,
  ) {
    (renderObject as _RenderHeaderFit)
      ..minimumWidth = minimumWidth
      ..horizontalInset = horizontalInset;
  }
}

class _HeaderStageScope extends InheritedWidget {
  const _HeaderStageScope({required this.stage, required super.child});

  final HeaderStage stage;

  @override
  bool updateShouldNotify(_HeaderStageScope oldWidget) =>
      oldWidget.stage != stage;
}

class _RenderHeaderFit extends RenderBox
    with
        RenderObjectWithChildMixin<RenderBox>,
        RenderObjectWithLayoutCallbackMixin,
        RenderAbstractLayoutBuilderMixin<HeaderStage, RenderBox> {
  _RenderHeaderFit({
    required double minimumWidth,
    required double horizontalInset,
  }) : _minimumWidth = minimumWidth,
       _horizontalInset = horizontalInset;

  _RenderHeaderFitRow? _row;
  HeaderStage _stage = HeaderStage.detailed;

  double _minimumWidth;
  set minimumWidth(double value) {
    if (value == _minimumWidth) return;
    _minimumWidth = value;
    markNeedsLayout();
  }

  double _horizontalInset;
  set horizontalInset(double value) {
    if (value == _horizontalInset) return;
    _horizontalInset = value;
    markNeedsLayout();
  }

  @override
  HeaderStage get layoutInfo => _stage;

  /// The row measures every stage from one build: hidden parts stay mounted
  /// and report the width they would take.
  HeaderStage _fit(double windowWidth) {
    if (windowWidth < _minimumWidth) return HeaderStage.bottomNavigation;
    final row = _row;
    if (row == null) return _stage;
    final available = windowWidth - _horizontalInset;
    for (final stage in HeaderStage.values) {
      if (!stage.showsHeaderNavigation) break;
      if (row.widthAt(stage) <= available) return stage;
    }
    return HeaderStage.bottomNavigation;
  }

  @override
  void performLayout() {
    final constraints = this.constraints;
    if (constraints.maxWidth < _minimumWidth) {
      _stage = HeaderStage.bottomNavigation;
    }
    runLayoutCallback();
    final fitted = _fit(constraints.maxWidth);
    if (fitted != _stage) {
      _stage = fitted;
      runLayoutCallback();
    }
    final child = this.child;
    if (child == null) {
      size = constraints.biggest;
      return;
    }
    child.layout(constraints, parentUsesSize: true);
    size = constraints.constrain(child.size);
  }

  @override
  Size computeDryLayout(BoxConstraints constraints) {
    assert(
      debugCannotComputeDryLayout(
        reason: 'The header stage is chosen by building during layout.',
      ),
    );
    return Size.zero;
  }

  @override
  bool hitTestChildren(BoxHitTestResult result, {required Offset position}) =>
      child?.hitTest(result, position: position) ?? false;

  @override
  void paint(PaintingContext context, Offset offset) {
    final child = this.child;
    if (child != null) context.paintChild(child, offset);
  }
}

/// Marks the header content whose width decides the [HeaderStage].
class HeaderFitRow extends SingleChildRenderObjectWidget {
  const HeaderFitRow({super.key, required Widget super.child});

  @override
  RenderObject createRenderObject(BuildContext context) =>
      _RenderHeaderFitRow();
}

class _RenderHeaderFitRow extends RenderProxyBox {
  final Set<_RenderHeaderCollapsible> _parts = {};
  _RenderHeaderFit? _fit;

  double widthAt(HeaderStage stage) {
    var width = getMaxIntrinsicWidth(double.infinity);
    for (final part in _parts) {
      width += part.widthChangeAt(stage);
    }
    return width;
  }

  @override
  void attach(PipelineOwner owner) {
    super.attach(owner);
    _fit = _ancestor<_RenderHeaderFit>(this)?.._row = this;
  }

  @override
  void detach() {
    if (identical(_fit?._row, this)) _fit!._row = null;
    _fit = null;
    super.detach();
  }

  /// A change in content width may change the stage that fits.
  @override
  void markNeedsLayout() {
    super.markNeedsLayout();
    _fit?.markNeedsLayout();
  }
}

/// The part or row that measures [object]: its nearest enclosing one.
RenderObject? _measuringAncestor(RenderObject object) {
  for (var node = object.parent; node != null; node = node.parent) {
    if (node is _RenderHeaderCollapsible || node is _RenderHeaderFitRow) {
      return node;
    }
  }
  return null;
}

T? _ancestor<T extends RenderObject>(RenderObject object) {
  for (var node = object.parent; node != null; node = node.parent) {
    if (node is T) return node;
  }
  return null;
}

/// Header content shown only from stage [from] up to, not including, [until].
///
/// A hidden part stays mounted so the header can measure it, but takes no
/// space, input, focus or semantics. A part inside another one is measured
/// within it.
class HeaderCollapsible extends StatelessWidget {
  const HeaderCollapsible({
    super.key,
    this.from = HeaderStage.detailed,
    this.until,
    required this.child,
  });

  final HeaderStage from;
  final HeaderStage? until;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final stage = HeaderFit.stageOf(context);
    final visible = _visibleAt(stage, from, until);
    return _HeaderCollapsibleBox(
      from: from,
      until: until,
      offstage: !visible,
      child: ExcludeFocus(excluding: !visible, child: child),
    );
  }
}

bool _visibleAt(HeaderStage stage, HeaderStage from, HeaderStage? until) =>
    stage.index >= from.index && (until == null || stage.index < until.index);

class _HeaderCollapsibleBox extends SingleChildRenderObjectWidget {
  const _HeaderCollapsibleBox({
    required this.from,
    required this.until,
    required this.offstage,
    required Widget super.child,
  });

  final HeaderStage from;
  final HeaderStage? until;
  final bool offstage;

  @override
  RenderObject createRenderObject(BuildContext context) =>
      _RenderHeaderCollapsible(from: from, until: until, offstage: offstage);

  @override
  void updateRenderObject(
    BuildContext context,
    covariant RenderObject renderObject,
  ) {
    (renderObject as _RenderHeaderCollapsible)
      ..from = from
      ..until = until
      ..offstage = offstage;
  }

  @override
  SingleChildRenderObjectElement createElement() =>
      _HeaderCollapsibleElement(this);
}

class _HeaderCollapsibleElement extends SingleChildRenderObjectElement {
  _HeaderCollapsibleElement(_HeaderCollapsibleBox super.widget);

  @override
  void debugVisitOnstageChildren(ElementVisitor visitor) {
    if (!(widget as _HeaderCollapsibleBox).offstage) {
      super.debugVisitOnstageChildren(visitor);
    }
  }
}

class _RenderHeaderCollapsible extends RenderOffstage {
  _RenderHeaderCollapsible({
    required this.from,
    required this.until,
    required super.offstage,
  });

  HeaderStage from;
  HeaderStage? until;
  final Set<_RenderHeaderCollapsible> _parts = {};
  Set<_RenderHeaderCollapsible>? _owner;

  double get naturalWidth => child?.getMaxIntrinsicWidth(double.infinity) ?? 0;

  bool visibleAt(HeaderStage stage) => _visibleAt(stage, from, until);

  /// How the width of the enclosing content changes from now to [stage].
  /// [naturalWidth] already holds the inner parts as they are now.
  double widthChangeAt(HeaderStage stage) {
    final natural = naturalWidth;
    var change = offstage ? 0.0 : -natural;
    if (visibleAt(stage)) {
      change += natural;
      for (final part in _parts) {
        change += part.widthChangeAt(stage);
      }
    }
    return change;
  }

  @override
  void attach(PipelineOwner owner) {
    super.attach(owner);
    _owner = switch (_measuringAncestor(this)) {
      final _RenderHeaderCollapsible part => part._parts,
      final _RenderHeaderFitRow row => row._parts,
      _ => null,
    }?..add(this);
  }

  @override
  void detach() {
    _owner?.remove(this);
    _owner = null;
    super.detach();
  }
}
