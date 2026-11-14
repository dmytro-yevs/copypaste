import 'package:copypaste_flutter/features/devices/devices_gateway.dart';
import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:copypaste_flutter/shared/clip_visibility.dart';
import 'package:flutter/foundation.dart';

/// The mutually exclusive filter and presentation kinds exposed by Rust.
enum HistoryClipKind {
  text,
  link,
  email,
  color,
  phone,
  code,
  json,
  path,
  credentials,
  image,
  file,
  other,
}

extension HistoryClipKindX on HistoryClipKind {
  String get label {
    final strings = AppStrings.current;
    return switch (this) {
      HistoryClipKind.text => strings.historyKindText,
      HistoryClipKind.link => strings.historyKindLink,
      HistoryClipKind.email => strings.historyKindEmail,
      HistoryClipKind.color => strings.historyKindColor,
      HistoryClipKind.phone => strings.historyKindPhone,
      HistoryClipKind.code => strings.historyKindCode,
      HistoryClipKind.json => 'JSON',
      HistoryClipKind.path => strings.historyKindPath,
      HistoryClipKind.credentials => strings.historyKindCredentials,
      HistoryClipKind.image => strings.historyKindImage,
      HistoryClipKind.file => strings.historyKindFile,
      HistoryClipKind.other => strings.historyKindOther,
    };
  }

  bool get isTextual => switch (this) {
    HistoryClipKind.text ||
    HistoryClipKind.link ||
    HistoryClipKind.email ||
    HistoryClipKind.color ||
    HistoryClipKind.phone ||
    HistoryClipKind.code ||
    HistoryClipKind.json ||
    HistoryClipKind.path ||
    HistoryClipKind.credentials => true,
    HistoryClipKind.image ||
    HistoryClipKind.file ||
    HistoryClipKind.other => false,
  };

  static HistoryClipKind fromContentType(String contentType) {
    final type = contentType.toLowerCase();
    if (type == 'text' || type.startsWith('text/')) return HistoryClipKind.text;
    if (type == 'file') return HistoryClipKind.file;
    if (type.startsWith('image/')) return HistoryClipKind.image;
    return HistoryClipKind.other;
  }
}

enum HistorySort { newest, oldest, relevance }

extension HistorySortX on HistorySort {
  String get label => switch (this) {
    HistorySort.newest => AppStrings.current.historySortNewest,
    HistorySort.oldest => AppStrings.current.historySortOldest,
    HistorySort.relevance => AppStrings.current.historySortRelevance,
  };
}

/// A server-owned query. All filters are evaluated before keyset paging.
class HistoryQuery {
  const HistoryQuery({
    this.search = '',
    this.kind,
    this.pinnedOnly = false,
    this.origin,
    this.sourceApp,
    this.sort = HistorySort.newest,
    this.capturedFrom,
    this.capturedBefore,
    this.tagIds = const [],
    this.untaggedOnly = false,
  });

  final String search;
  final HistoryClipKind? kind;
  final bool pinnedOnly;
  final String? origin;
  final String? sourceApp;
  final HistorySort sort;
  final DateTime? capturedFrom;
  final DateTime? capturedBefore;
  final List<String> tagIds;
  final bool untaggedOnly;
  bool get hasDateFilter => capturedFrom != null || capturedBefore != null;
  bool get hasTagFilter => tagIds.isNotEmpty || untaggedOnly;

  bool get hasSearch => search.trim().isNotEmpty;

  HistoryQuery copyWith({
    String? search,
    HistoryClipKind? kind,
    bool clearKind = false,
    bool? pinnedOnly,
    String? origin,
    bool clearOrigin = false,
    String? sourceApp,
    bool clearSourceApp = false,
    HistorySort? sort,
    DateTime? capturedFrom,
    DateTime? capturedBefore,
    bool clearDates = false,
    List<String>? tagIds,
    bool? untaggedOnly,
  }) {
    return HistoryQuery(
      search: search ?? this.search,
      kind: clearKind ? null : kind ?? this.kind,
      pinnedOnly: pinnedOnly ?? this.pinnedOnly,
      origin: clearOrigin ? null : origin ?? this.origin,
      sourceApp: clearSourceApp ? null : sourceApp ?? this.sourceApp,
      sort: sort ?? this.sort,
      capturedFrom: clearDates ? null : capturedFrom ?? this.capturedFrom,
      capturedBefore: clearDates ? null : capturedBefore ?? this.capturedBefore,
      tagIds: tagIds ?? this.tagIds,
      untaggedOnly: untaggedOnly ?? this.untaggedOnly,
    );
  }
}

class HistoryFileDetails {
  const HistoryFileDetails({
    this.name,
    this.mimeType,
    this.sourceReference,
    this.sourceAvailable = false,
    this.sizeBytes,
    this.fileCount,
  });

  final String? name;
  final String? mimeType;
  final String? sourceReference;
  final bool sourceAvailable;
  final int? sizeBytes;
  final int? fileCount;
}

class HistoryImageDetails {
  const HistoryImageDetails({
    required this.width,
    required this.height,
    required this.sizeBytes,
  });

  final int width;
  final int height;
  final int sizeBytes;
}

/// A backend-provided filter choice. [id] is never rendered in the UI.
sealed class HistoryFilterFacet {
  const HistoryFilterFacet({required this.id, required this.label});

  final String id;
  final String label;
}

class HistoryDeviceFacet extends HistoryFilterFacet {
  const HistoryDeviceFacet({
    required super.id,
    required super.label,
    required this.deviceClass,
  });

  final DeviceClass deviceClass;
}

class HistorySourceAppFacet extends HistoryFilterFacet {
  const HistorySourceAppFacet({
    required super.id,
    required super.label,
    this.iconId,
  });

  final String? iconId;
}

class HistoryFacets {
  const HistoryFacets({
    this.originDevices = const [],
    this.sourceApps = const [],
    this.tags = const [],
  });

  final List<HistoryDeviceFacet> originDevices;
  final List<HistorySourceAppFacet> sourceApps;
  final List<HistoryTag> tags;
}

/// How CopyPaste itself created a clip, shown next to its source app.
enum HistoryProvenance { manual, edited, transformed, module }

/// Runtime state attached to a clip beyond its captured payload.
@immutable
class HistoryClipAnnotations {
  const HistoryClipAnnotations({
    this.expiresAt,
    this.ruleSecret = false,
    this.provenance,
    this.ruleTagIds = const [],
    this.keep = false,
  });

  static const none = HistoryClipAnnotations();

  /// When an expiring clip is deleted; null without expiry.
  final DateTime? expiresAt;

  /// A local display mask set by a content rule.
  final bool ruleSecret;
  final HistoryProvenance? provenance;

  /// Tags a content rule assigned; they are never synced memberships.
  final List<String> ruleTagIds;

  /// The user chose to keep the clip, which cancels any expiry.
  final bool keep;

  @override
  bool operator ==(Object other) =>
      other is HistoryClipAnnotations &&
      other.expiresAt == expiresAt &&
      other.ruleSecret == ruleSecret &&
      other.provenance == provenance &&
      listEquals(other.ruleTagIds, ruleTagIds) &&
      other.keep == keep;

  @override
  int get hashCode => Object.hash(
    expiresAt,
    ruleSecret,
    provenance,
    Object.hashAll(ruleTagIds),
    keep,
  );
}

/// Compact item data returned by a page query.
class HistoryClip {
  const HistoryClip({
    this.secret = false,
    this.transient = false,
    required this.id,
    required this.contentType,
    required this.preview,
    required this.createdAt,
    required this.pinned,
    this.tagIds = const [],
    this.kind,
    this.origin,
    this.originDeviceClass = DeviceClass.unknown,
    this.sourceApp,
    this.sourceAppIconId,
    this.truncated = false,
    this.lineCount,
    this.body,
    this.file,
    this.image,
    this.colorRgba,
    this.tooLargeToSync = false,
    this.annotations = HistoryClipAnnotations.none,
  });

  /// The authenticated secret flag the user or the source app set.
  final bool secret;
  final bool transient;
  final String id;
  final String contentType;
  final String preview;
  final DateTime createdAt;
  final bool pinned;

  /// Name-ordered tags of a pinned clip, bounded per page item by Rust.
  final List<String> tagIds;
  final HistoryClipKind? kind;
  final String? origin;
  final DeviceClass originDeviceClass;
  final String? sourceApp;
  final String? sourceAppIconId;
  final bool truncated;

  /// Lines of the whole text, counted by the runtime for public text clips.
  final int? lineCount;

  /// Present only after an explicit get operation for the selected item.
  final String? body;
  final HistoryFileDetails? file;
  final HistoryImageDetails? image;

  /// Packed as `0xRRGGBBAA` for color clips.
  final int? colorRgba;
  final bool tooLargeToSync;
  final HistoryClipAnnotations annotations;

  HistoryClipKind get contentKind =>
      kind ?? HistoryClipKindX.fromContentType(contentType);

  ClipVisibility get visibility => ClipVisibility(
    secret: secret,
    ruleSecret: annotations.ruleSecret,
    expiresAt: annotations.expiresAt,
  );

  /// Shown behind a spoiler: secret, or masked by a content rule.
  bool get masked => visibility.masked;

  HistoryClip copyWith({
    bool? pinned,
    bool? secret,
    String? preview,
    bool? truncated,
    bool clearBody = false,
    HistoryClipAnnotations? annotations,
  }) => HistoryClip(
    secret: secret ?? this.secret,
    transient: transient,
    id: id,
    contentType: contentType,
    preview: preview ?? this.preview,
    createdAt: createdAt,
    pinned: pinned ?? this.pinned,
    tagIds: pinned == false ? const [] : tagIds,
    kind: kind,
    origin: origin,
    originDeviceClass: originDeviceClass,
    sourceApp: sourceApp,
    sourceAppIconId: sourceAppIconId,
    truncated: truncated ?? this.truncated,
    lineCount: lineCount,
    body: clearBody ? null : body,
    file: file,
    image: image,
    colorRgba: colorRgba,
    tooLargeToSync: tooLargeToSync,
    annotations: annotations ?? this.annotations,
  );
}

class HistoryClipPage {
  const HistoryClipPage({
    required this.items,
    this.nextCursor,
    this.skippedUndecryptable = 0,
  });

  final List<HistoryClip> items;
  final String? nextCursor;
  final int skippedUndecryptable;
}

/// Exact sizes of the groups the History list shows for one query, counted by
/// the runtime rather than from loaded pages.
@immutable
class HistoryGroupCounts {
  HistoryGroupCounts({required this.pinned, required Map<DateTime, int> days})
    : _days = Map.unmodifiable(days);

  /// Pinned clips matching the query.
  final int pinned;
  final Map<DateTime, int> _days;

  /// Unpinned clips captured on the local calendar day of [date]; null when
  /// that day was not counted, which is not the same as zero.
  int? day(DateTime date) {
    final local = date.toLocal();
    return _days[DateTime(local.year, local.month, local.day)];
  }

  @override
  bool operator ==(Object other) =>
      other is HistoryGroupCounts &&
      other.pinned == pinned &&
      mapEquals(other._days, _days);

  @override
  int get hashCode => Object.hash(
    pinned,
    Object.hashAllUnordered(
      _days.entries.map((entry) => Object.hash(entry.key, entry.value)),
    ),
  );
}

/// Physical-pixel limits for an aspect-preserving preview of the stored image.
class HistoryImagePreviewBounds {
  const HistoryImagePreviewBounds({required this.width, required this.height});

  final int width;
  final int height;
}

class HistoryImagePreview {
  const HistoryImagePreview(
    this.bytes, {
    required this.width,
    required this.height,
  });

  final Uint8List bytes;
  final int width;
  final int height;
}

class HistorySourceAppIcon {
  const HistorySourceAppIcon(this.bytes, {this.width, this.height});

  final Uint8List bytes;
  final int? width;
  final int? height;
}

enum HistoryRuntimeEvent { itemsChanged, unavailable }

class HistoryTag {
  const HistoryTag({required this.id, required this.name});
  final String id;
  final String name;
}

class HistoryTags {
  const HistoryTags({this.tags = const [], this.assignments = const {}});
  final List<HistoryTag> tags;
  final Map<String, List<String>> assignments;
}
