import '../models/history_models.dart';

/// Page-local cursor and scalar row metadata. The durable ledger deliberately
/// keeps no preview/body/file summaries after a page is evicted.
class HistoryWindowPage {
  HistoryWindowPage({required this.inputCursor, required HistoryClipPage page})
    : nextCursor = page.nextCursor,
      ids = List.unmodifiable(page.items.map((item) => item.id)),
      createdAtMillis = List.unmodifiable(
        page.items.map((item) => item.createdAt.millisecondsSinceEpoch),
      ),
      pinnedIds = List.unmodifiable(
        page.items.where((item) => item.pinned).map((item) => item.id),
      ),
      skippedUndecryptable = page.skippedUndecryptable,
      items = page.items;

  final String? inputCursor;
  String? nextCursor;
  List<String> ids;
  List<int> createdAtMillis;
  List<String> pinnedIds;
  int skippedUndecryptable;
  List<HistoryClip>? items;
  double? extent;
  double? pinnedExtent;
  double? recentExtent;

  bool get hasExactExtent =>
      extent != null || (pinnedExtent != null && recentExtent != null);

  HistoryWindowPage ledgerCopy() {
    final copy = HistoryWindowPage(
      inputCursor: inputCursor,
      page: HistoryClipPage(
        items: items ?? const [],
        nextCursor: nextCursor,
        skippedUndecryptable: skippedUndecryptable,
      ),
    );
    copy.ids = ids;
    copy.createdAtMillis = createdAtMillis;
    copy.pinnedIds = pinnedIds;
    copy.items = items;
    copy.extent = extent;
    copy.pinnedExtent = pinnedExtent;
    copy.recentExtent = recentExtent;
    return copy;
  }

  bool get resident => items != null;
}

/// Bounds retained History summaries. The ledger can grow with visited IDs,
/// but its entries are scalar cursors, IDs, pin flags, and dates only.
class HistoryWindowIndex {
  HistoryWindowIndex({
    required this.maxPages,
    required this.maxBytes,
    this.independentPins = false,
  });

  final int maxPages;
  final int maxBytes;
  final bool independentPins;
  final List<HistoryWindowPage> pages = [];

  Iterable<String> get ids => pages.expand((page) => page.ids);
  Iterable<String> get pinnedIds => pages.expand((page) => page.pinnedIds);
  int get residentPageCount => pages.where((page) => page.resident).length;
  int get residentBytes =>
      pages.fold(0, (total, page) => total + _bytes(page.items));
  bool get exceedsLimits {
    if (!independentPins) {
      return residentPageCount > maxPages || residentBytes > maxBytes;
    }
    final resident = pages.where((page) => page.resident);
    final pins = resident
        .where((page) => page.items!.any((item) => item.pinned))
        .length;
    final recent = resident
        .where((page) => page.items!.any((item) => !item.pinned))
        .length;
    return pins > maxPages || recent > maxPages || residentBytes > maxBytes * 2;
  }

  void clear() => pages.clear();

  void append(String? inputCursor, HistoryClipPage page) {
    pages.add(HistoryWindowPage(inputCursor: inputCursor, page: page));
  }

  void replace(int index, HistoryClipPage page) {
    final previous = pages[index];
    pages[index] = HistoryWindowPage(
      inputCursor: previous.inputCursor,
      page: page,
    );
    if (previous.nextCursor != page.nextCursor) {
      pages.removeRange(index + 1, pages.length);
    }
  }

  void removeId(String id) {
    for (final page in pages) {
      final row = page.ids.indexOf(id);
      if (row < 0) continue;
      page.ids = List.unmodifiable([...page.ids]..removeAt(row));
      page.createdAtMillis = List.unmodifiable(
        [...page.createdAtMillis]..removeAt(row),
      );
      page.pinnedIds = List.unmodifiable(
        page.pinnedIds.where((value) => value != id),
      );
      page.items = page.items
          ?.where((item) => item.id != id)
          .toList(growable: false);
    }
  }

  void replaceItem(HistoryClip item) {
    for (final page in pages) {
      final row = page.ids.indexOf(item.id);
      if (row < 0) continue;
      final source = page.items;
      final index = source?.indexWhere((candidate) => candidate.id == item.id);
      if (index != null && index >= 0) {
        page.items = List.of(source!)..[index] = item;
      }
      // Pin metadata covers the whole page, including either evicted lane.
      final pins = page.pinnedIds.toSet();
      if (item.pinned) {
        pins.add(item.id);
      } else {
        pins.remove(item.id);
      }
      page.pinnedIds = List.unmodifiable([
        for (final id in page.ids)
          if (pins.contains(id)) id,
      ]);
      page.createdAtMillis = List.of(page.createdAtMillis)
        ..[row] = item.createdAt.millisecondsSinceEpoch;
      return;
    }
  }

  void retainPinnedOnly() {
    for (final page in pages) {
      final pinned = page.pinnedIds.toSet();
      final indexes = <int>[
        for (var i = 0; i < page.ids.length; i++)
          if (pinned.contains(page.ids[i])) i,
      ];
      page.ids = List.unmodifiable([for (final i in indexes) page.ids[i]]);
      page.createdAtMillis = List.unmodifiable([
        for (final i in indexes) page.createdAtMillis[i],
      ]);
      page.items = page.items
          ?.where((item) => item.pinned)
          .toList(growable: false);
    }
    pages.removeWhere((page) => page.ids.isEmpty);
  }

  /// Keeps the active page plus its two nearest neighbours. A single oversized
  /// active page is allowed temporarily; the next incoming page evicts it.
  void retainAround(int activeIndex, {Set<String> protectedIds = const {}}) {
    while (exceedsLimits) {
      final resident = <int>[
        for (var i = 0; i < pages.length; i++)
          if (pages[i].resident) i,
      ];
      final pinnedCount = resident
          .where((i) => pages[i].items!.any((item) => item.pinned))
          .length;
      final recentCount = resident
          .where((i) => pages[i].items!.any((item) => !item.pinned))
          .length;
      // Trim one lane per pass. If both lanes exceed the cap, requiring a
      // mixed-page victim would retain every old single-lane page forever.
      final trimPins = independentPins && pinnedCount > maxPages;
      final trimRecent = independentPins && !trimPins && recentCount > maxPages;
      int? victim;
      var farthest = -1;
      for (final i in resident) {
        final page = pages[i];
        final rows = page.items;
        final protected = rows
            ?.where((item) => protectedIds.contains(item.id))
            .toList(growable: false);
        final protectedSurvives = trimRecent
            ? protected?.every((item) => item.pinned) ?? true
            : trimPins
            ? protected?.every((item) => !item.pinned) ?? true
            : false;
        if (rows == null ||
            (!independentPins && !page.hasExactExtent) ||
            i == activeIndex ||
            (protected?.isNotEmpty == true && !protectedSurvives))
          continue;
        if (trimPins && !rows.any((item) => item.pinned)) continue;
        if (trimRecent && !rows.any((item) => !item.pinned)) continue;
        final distance = (i - activeIndex).abs();
        if (distance > farthest) {
          farthest = distance;
          victim = i;
        }
      }
      if (victim == null) return;
      final items = pages[victim].items!;
      if (trimPins && items.any((item) => !item.pinned)) {
        pages[victim].items = items
            .where((item) => !item.pinned)
            .toList(growable: false);
      } else if (trimRecent && items.any((item) => item.pinned)) {
        pages[victim].items = items
            .where((item) => item.pinned)
            .toList(growable: false);
      } else {
        pages[victim].items = null;
      }
    }
  }

  static int _bytes(List<HistoryClip>? items) {
    if (items == null) return 0;
    return items.fold(0, (total, item) {
      final file = item.file;
      return total +
          2 *
              (item.id.length +
                  item.contentType.length +
                  item.preview.length +
                  (item.origin?.length ?? 0) +
                  (item.sourceApp?.length ?? 0) +
                  (item.sourceAppIconId?.length ?? 0) +
                  (item.body?.length ?? 0) +
                  (file?.name?.length ?? 0) +
                  (file?.mimeType?.length ?? 0) +
                  (file?.sourceReference?.length ?? 0));
    });
  }
}
