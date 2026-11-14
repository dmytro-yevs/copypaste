import 'package:copypaste_flutter/generated/api.dart' as runtime;
import 'package:copypaste_flutter/generated/api/clips.dart' as clips;

import 'quick_paste_delivery_repository.dart';

/// [QuickPasteDeliveryRepository] over the runtime bridge.
class RuntimeQuickPasteDeliveryRepository
    implements QuickPasteDeliveryRepository {
  const RuntimeQuickPasteDeliveryRepository();

  @override
  Future<String?> deliverText(String text, {required bool secret}) => _guard(
    () async => (await clips.deliverText(
      text: text,
      secret: secret,
      delivery: clips.TextDelivery.paste,
    )).token,
  );

  @override
  Future<void> restoreClipboard(String token, Duration after) => _guard(
    () => clips.restoreClipboard(token: token, afterMs: after.inMilliseconds),
  );

  static Future<T> _guard<T>(Future<T> Function() request) async {
    try {
      return await request();
    } on runtime.RuntimeError catch (error) {
      throw QuickPasteDeliveryException(error.code, error.message);
    }
  }
}
