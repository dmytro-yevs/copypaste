import 'dart:typed_data';

import 'package:copypaste_flutter/features/devices/devices_gateway.dart';

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
  image,
  file,
  other,
}

extension HistoryClipKindX on HistoryClipKind {
  String get label => switch (this) {
    HistoryClipKind.text => 'Text',
    HistoryClipKind.link => 'Link',
    HistoryClipKind.email => 'Email',
    HistoryClipKind.color => 'Color',
    HistoryClipKind.phone => 'Phone',
    HistoryClipKind.code => 'Code',
    HistoryClipKind.json => 'JSON',
    HistoryClipKind.path => 'Path',
    HistoryClipKind.image => 'Image',
    HistoryClipKind.file => 'File',
    HistoryClipKind.other => 'Other',
  };

  bool get isTextual => switch (this) {
    HistoryClipKind.text ||
    HistoryClipKind.link ||
    HistoryClipKind.email ||
    HistoryClipKind.color ||
    HistoryClipKind.phone ||
    HistoryClipKind.code ||
    HistoryClipKind.json ||
    HistoryClipKind.path => true,
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
    HistorySort.newest => 'Newest first',
    HistorySort.oldest => 'Oldest first',
    HistorySort.relevance => 'Best match',
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
  });

  final String search;
  final HistoryClipKind? kind;
  final bool pinnedOnly;
  final String? origin;
  final String? sourceApp;
  final HistorySort sort;

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
  }) {
    return HistoryQuery(
      search: search ?? this.search,
      kind: clearKind ? null : kind ?? this.kind,
      pinnedOnly: pinnedOnly ?? this.pinnedOnly,
      origin: clearOrigin ? null : origin ?? this.origin,
      sourceApp: clearSourceApp ? null : sourceApp ?? this.sourceApp,
      sort: sort ?? this.sort,
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
    this.iconItemId,
  });

  final String? iconItemId;
}

class HistoryFacets {
  const HistoryFacets({
    this.originDevices = const [],
    this.sourceApps = const [],
  });

  final List<HistoryDeviceFacet> originDevices;
  final List<HistorySourceAppFacet> sourceApps;
}

/// Compact item data returned by a page query.
class HistoryClip {
  const HistoryClip({
    required this.id,
    required this.contentType,
    required this.preview,
    required this.createdAt,
    required this.pinned,
    this.kind,
    this.origin,
    this.originDeviceClass = DeviceClass.unknown,
    this.sourceApp,
    this.truncated = false,
    this.body,
    this.file,
    this.image,
    this.colorRgba,
    this.tooLargeToSync = false,
  });

  final String id;
  final String contentType;
  final String preview;
  final DateTime createdAt;
  final bool pinned;
  final HistoryClipKind? kind;
  final String? origin;
  final DeviceClass originDeviceClass;
  final String? sourceApp;
  final bool truncated;

  /// Present only after an explicit get operation for the selected item.
  final String? body;
  final HistoryFileDetails? file;
  final HistoryImageDetails? image;

  /// Packed as `0xRRGGBBAA` for color clips.
  final int? colorRgba;
  final bool tooLargeToSync;

  HistoryClipKind get contentKind =>
      kind ?? HistoryClipKindX.fromContentType(contentType);

  HistoryClip copyWith({bool? pinned}) => HistoryClip(
    id: id,
    contentType: contentType,
    preview: preview,
    createdAt: createdAt,
    pinned: pinned ?? this.pinned,
    kind: kind,
    origin: origin,
    originDeviceClass: originDeviceClass,
    sourceApp: sourceApp,
    truncated: truncated,
    body: body,
    file: file,
    image: image,
    colorRgba: colorRgba,
    tooLargeToSync: tooLargeToSync,
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
  const HistorySourceAppIcon(this.bytes);

  final Uint8List bytes;
}

enum HistoryRuntimeEvent { itemsChanged, unavailable }
