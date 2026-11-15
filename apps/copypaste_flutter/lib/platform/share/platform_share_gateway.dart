import 'dart:io' show Platform;
import 'dart:ui';

import 'package:share_plus/share_plus.dart';

/// The native share sheet: the Android chooser, the macOS sharing picker and
/// the Windows Share UI. [origin] anchors the picker in logical coordinates.
class PlatformShareGateway {
  const PlatformShareGateway();

  /// Whether one sheet takes text together with files. The macOS picker
  /// shares only the files when given both.
  bool get sharesTextWithFiles => Platform.isAndroid || Platform.isWindows;

  Future<void> shareText(String text, {Rect? origin}) =>
      _share(ShareParams(text: text, sharePositionOrigin: origin));

  Future<void> shareFile(String path, {String? mimeType, Rect? origin}) =>
      _share(
        ShareParams(
          files: [XFile(path, mimeType: mimeType)],
          sharePositionOrigin: origin,
        ),
      );

  /// Shares [paths] in order, with [text] where [sharesTextWithFiles].
  Future<void> shareFiles(List<String> paths, {String? text, Rect? origin}) =>
      _share(
        ShareParams(
          files: [for (final path in paths) XFile(path)],
          text: sharesTextWithFiles ? text : null,
          sharePositionOrigin: origin,
        ),
      );

  Future<void> _share(ShareParams params) async {
    await SharePlus.instance.share(params);
  }
}
