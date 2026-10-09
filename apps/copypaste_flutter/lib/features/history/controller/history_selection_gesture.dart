import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';

import '../../../app/theme/app_motion.dart';
import '../../../app/theme/app_tokens.dart';
import 'history_controller.dart';

/// Keeps a held selection alive as its original virtualized row scrolls away.
/// Button owns gesture recognition; this adapter owns pointer tracking and the
/// framework's edge auto-scroller for that same pointer session.
class HistorySelectionGesture {
  HistorySelectionGesture({
    required this.controller,
    required this.clipAt,
    required this.orderedIds,
    required this.viewportBounds,
  }) {
    controller.addListener(_changed);
  }

  final HistoryController controller;
  final String? Function(Offset position) clipAt;
  final List<String> Function() orderedIds;
  final Rect? Function() viewportBounds;
  EdgeDraggingAutoScroller? _autoScroller;
  int? _pointer;
  Offset? _position;
  bool _moved = false;
  bool _framePending = false;
  bool _disposed = false;

  void pointerDown(PointerDownEvent event) {
    if (_pointer != null || event.buttons & kPrimaryButton == 0) return;
    _pointer = event.pointer;
    _position = event.position;
  }

  void pointerMove(PointerMoveEvent event) {
    if (event.pointer != _pointer || !controller.isBulkDragSelecting) return;
    _position = event.position;
    _moved = true;
    _scheduleUpdate();
  }

  void pointerUp(PointerUpEvent event) {
    if (event.pointer == _pointer) end();
  }

  void pointerCancel(PointerCancelEvent event) {
    if (event.pointer == _pointer) end();
  }

  void begin(String id, Offset position, ScrollableState scrollable) {
    if (_disposed ||
        _pointer == null ||
        !controller.beginBulkDragSelection(id)) {
      return;
    }
    _position = position;
    _moved = false;
    _autoScroller?.stopAutoScroll();
    _autoScroller = EdgeDraggingAutoScroller(
      scrollable,
      velocityScalar: AppMotion.selectionScrollVelocityScalar,
      onScrollViewScrolled: updateAfterScroll,
    );
    _scheduleUpdate();
  }

  void _changed() {
    if (!controller.isBulkDragSelecting) {
      _autoScroller?.stopAutoScroll();
      return;
    }
    _scheduleUpdate();
  }

  void updateAfterScroll() {
    if (!controller.isBulkDragSelecting) return;
    _moved = true;
    _scheduleUpdate();
  }

  void _scheduleUpdate() {
    if (_disposed || _framePending || !controller.isBulkDragSelecting) return;
    _framePending = true;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _framePending = false;
      if (_disposed || !controller.isBulkDragSelecting) return;
      final position = _position;
      if (position == null) return;
      final visible = viewportBounds();
      if (visible == null || visible.isEmpty) {
        _autoScroller?.stopAutoScroll();
        return;
      }
      final id = _moved ? clipAt(position) : null;
      if (id != null) {
        controller.updateBulkDragSelection(id, orderedIds: orderedIds());
      }
      final scrollable = _autoScroller?.scrollable;
      final viewport = scrollable?.context.findRenderObject();
      if (viewport is! RenderBox ||
          !viewport.attached ||
          !viewport.hasSize ||
          viewport.size.isEmpty) {
        return;
      }
      final bottomClearance = math.max(
        0.0,
        viewport.localToGlobal(Offset.zero).dy +
            viewport.size.height -
            visible.bottom,
      );
      if (position.dy >= visible.bottom - AppControlSize.touch &&
          scrollable!.position.extentAfter <=
              AppControlSize.touch + bottomClearance &&
          controller.canLoadMore) {
        unawaited(controller.loadMore());
      }
      _autoScroller?.startAutoScrollIfNecessary(
        Rect.fromCenter(
          center: position + Offset(0, bottomClearance / 2),
          width: math.min(AppControlSize.touch, viewport.size.width),
          height: math.min(
            AppControlSize.touch * 2 + bottomClearance,
            viewport.size.height,
          ),
        ),
      );
    });
    WidgetsBinding.instance.ensureVisualUpdate();
  }

  void end() {
    _autoScroller?.stopAutoScroll();
    _autoScroller = null;
    _pointer = null;
    _position = null;
    _moved = false;
    controller.endBulkDragSelection();
  }

  void dispose() {
    _disposed = true;
    controller.removeListener(_changed);
    end();
  }
}
