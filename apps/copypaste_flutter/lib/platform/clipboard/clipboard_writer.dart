import 'package:flutter/services.dart';

abstract interface class ClipboardWriter {
  Future<void> writeText(String text);
}

class SystemClipboardWriter implements ClipboardWriter {
  const SystemClipboardWriter();

  @override
  Future<void> writeText(String text) =>
      Clipboard.setData(ClipboardData(text: text));
}
