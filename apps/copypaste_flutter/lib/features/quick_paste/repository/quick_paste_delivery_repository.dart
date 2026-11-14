/// Clipboard deliveries Quick Paste makes besides copying a stored clip.
///
/// Failures are [QuickPasteDeliveryException]s carrying the runtime code.
abstract interface class QuickPasteDeliveryRepository {
  /// Puts produced text on the clipboard for a paste. It creates no history
  /// row, promotes nothing and records no snippet usage. Returns the token
  /// that restores the previous clipboard, when there is one.
  Future<String?> deliverText(String text, {required bool secret});

  /// Puts back what the clipboard held before [token]'s write, after
  /// [after], unless the clipboard changed meanwhile.
  Future<void> restoreClipboard(String token, Duration after);
}

class QuickPasteDeliveryException implements Exception {
  const QuickPasteDeliveryException(this.code, [this.message = '']);

  final String code;
  final String message;

  @override
  String toString() => 'QuickPasteDeliveryException($code)';
}
