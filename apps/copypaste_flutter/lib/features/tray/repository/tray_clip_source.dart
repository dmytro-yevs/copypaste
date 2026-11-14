import '../models/tray_menu_model.dart';

/// The runtime reads and writes the tray menu needs.
abstract interface class TrayClipSource {
  /// The newest clips, already filtered and masked for presentation.
  Future<List<TrayClip>> recent(int limit);

  Future<TrayClip?> latestFromOtherDevice();

  /// Copies a stored clip as an own write, which promotes it.
  Future<void> copy(String id);

  /// Stores the clipboard's current text; false when it holds none.
  Future<bool> captureClipboard();

  Future<List<TraySkipEntry>> recentlySkipped();
}
