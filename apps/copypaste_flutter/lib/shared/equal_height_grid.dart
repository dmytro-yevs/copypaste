import 'dart:math' as math;

import 'package:flutter/rendering.dart';
import 'package:flutter/widgets.dart';

/// Fits cards to the tallest content without clipping text or fixing a height.
class EqualHeightGrid extends MultiChildRenderObjectWidget {
  const EqualHeightGrid({
    super.key,
    required this.minChildWidth,
    required this.spacing,
    this.maxColumns = 3,
    required super.children,
  });

  final double minChildWidth;
  final double spacing;
  final int maxColumns;

  @override
  RenderObject createRenderObject(BuildContext context) =>
      _RenderEqualHeightGrid(
        minChildWidth,
        spacing,
        maxColumns,
        Directionality.of(context),
      );

  @override
  void updateRenderObject(
    BuildContext context,
    covariant RenderBox renderObject,
  ) {
    (renderObject as _RenderEqualHeightGrid)
      ..minChildWidth = minChildWidth
      ..spacing = spacing
      ..maxColumns = maxColumns
      ..textDirection = Directionality.of(context)
      ..markNeedsLayout();
  }
}

class _GridParentData extends ContainerBoxParentData<RenderBox> {}

class _RenderEqualHeightGrid extends RenderBox
    with
        ContainerRenderObjectMixin<
          RenderBox,
          ContainerBoxParentData<RenderBox>
        >,
        RenderBoxContainerDefaultsMixin<
          RenderBox,
          ContainerBoxParentData<RenderBox>
        > {
  _RenderEqualHeightGrid(
    this.minChildWidth,
    this.spacing,
    this.maxColumns,
    this.textDirection,
  );

  double minChildWidth;
  double spacing;
  int maxColumns;
  TextDirection textDirection;

  @override
  void setupParentData(RenderBox child) {
    if (child.parentData is! ContainerBoxParentData<RenderBox>) {
      child.parentData = _GridParentData();
    }
  }

  @override
  void performLayout() {
    final width = constraints.maxWidth;
    final columns = ((width + spacing) / (minChildWidth + spacing))
        .floor()
        .clamp(1, maxColumns);
    final childWidth = (width - spacing * (columns - 1)) / columns;
    final naturalConstraints = BoxConstraints.tightFor(width: childWidth);
    var height = 0.0;
    var child = firstChild;
    while (child != null) {
      child.layout(naturalConstraints, parentUsesSize: true);
      height = math.max(height, child.size.height);
      child = childAfter(child);
    }
    final equalConstraints = BoxConstraints.tight(Size(childWidth, height));
    var index = 0;
    child = firstChild;
    while (child != null) {
      child.layout(equalConstraints, parentUsesSize: true);
      final column = index % columns;
      final left = column * (childWidth + spacing);
      (child.parentData! as ContainerBoxParentData<RenderBox>).offset = Offset(
        textDirection == TextDirection.ltr ? left : width - childWidth - left,
        (index ~/ columns) * (height + spacing),
      );
      index++;
      child = childAfter(child);
    }
    final rows = (childCount / columns).ceil();
    size = constraints.constrain(
      Size(width, rows == 0 ? 0 : rows * height + (rows - 1) * spacing),
    );
  }

  @override
  void paint(PaintingContext context, Offset offset) =>
      defaultPaint(context, offset);

  @override
  bool hitTestChildren(BoxHitTestResult result, {required Offset position}) =>
      defaultHitTestChildren(result, position: position);
}
