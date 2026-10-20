import 'package:flutter/rendering.dart';
import 'package:flutter/widgets.dart';

class HistoryWindowPageView {
  const HistoryWindowPageView({
    required this.id,
    required this.extent,
    required this.rows,
    required this.rowBuilder,
    required this.onMeasured,
    required this.onPlaceholderVisible,
  });

  final Object id;
  final double? extent;
  final int rows;
  final IndexedWidgetBuilder? rowBuilder;
  final ValueChanged<double> onMeasured;
  final VoidCallback onPlaceholderVisible;
  bool get resident => rowBuilder != null;
}

/// Replaces a resident page with its latest sliver scroll extent. SliverList
/// may estimate unseen rows; visible-row anchors are measured separately and
/// corrected when a refetched page changes geometry.
class HistoryWindowScroll extends StatelessWidget {
  const HistoryWindowScroll({
    super.key,
    required this.controller,
    required this.pages,
    this.viewportKey,
    this.bottomPadding = 0,
    this.trailing,
  });

  final ScrollController controller;
  final List<HistoryWindowPageView> pages;
  final Key? viewportKey;
  final double bottomPadding;
  final Widget? trailing;

  @override
  Widget build(BuildContext context) {
    return CustomScrollView(
      key: viewportKey,
      controller: controller,
      primary: false,
      slivers: [
        for (final page in pages) _page(page),
        if (trailing != null) SliverToBoxAdapter(child: trailing),
        SliverToBoxAdapter(child: SizedBox(height: bottomPadding)),
      ],
    );
  }

  Widget _page(HistoryWindowPageView page) {
    if (page.resident) {
      return _MeasuredSliverList(
        key: ValueKey<Object>('history-page-${page.id}'),
        childCount: page.rows,
        builder: page.rowBuilder!,
        onMeasured: page.onMeasured,
      );
    }
    final extent = page.extent;
    assert(extent != null && extent >= 0, 'Only measured pages may be evicted');
    return SliverLayoutBuilder(
      key: ValueKey<Object>('history-page-placeholder-${page.id}'),
      builder: (context, constraints) {
        if (constraints.scrollOffset < extent! &&
            constraints.scrollOffset + constraints.remainingCacheExtent > 0) {
          WidgetsBinding.instance.addPostFrameCallback((_) {
            page.onPlaceholderVisible();
          });
        }
        return SliverToBoxAdapter(child: SizedBox(height: extent));
      },
    );
  }
}

class _MeasuredSliverList extends StatefulWidget {
  const _MeasuredSliverList({
    super.key,
    required this.childCount,
    required this.builder,
    required this.onMeasured,
  });

  final int childCount;
  final IndexedWidgetBuilder builder;
  final ValueChanged<double> onMeasured;

  @override
  State<_MeasuredSliverList> createState() => _MeasuredSliverListState();
}

class _MeasuredSliverListState extends State<_MeasuredSliverList> {
  double? _reported;

  void _report() {
    final render = context.findRenderObject();
    final extent = render is RenderSliver
        ? render.geometry?.scrollExtent
        : null;
    if (extent == null || extent < 0 || extent == _reported) return;
    _reported = extent;
    widget.onMeasured(extent);
  }

  @override
  Widget build(BuildContext context) {
    WidgetsBinding.instance.addPostFrameCallback((_) => _report());
    return SliverList(
      delegate: SliverChildBuilderDelegate(
        widget.builder,
        childCount: widget.childCount,
      ),
    );
  }
}
