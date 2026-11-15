import '../../../generated/api/clips.dart' as clips;
import '../models/module_output.dart';
import 'runtime_modules_repository.dart';

/// What a delivery left on the clipboard.
typedef ModuleDelivered = ({String? token, int? cursorFromEnd});

/// Puts text a module delivers on the clipboard.
abstract interface class ModuleTextDelivery {
  /// Writes [delivery] of module [moduleId] transiently, secret text as a
  /// secret, for a paste or a copy. Non-secret text is recorded in history
  /// with the module's provenance when the delivery asks to. The token
  /// restores the previous clipboard.
  Future<ModuleDelivered> deliver(
    String moduleId,
    ModuleDelivery delivery, {
    required bool paste,
  });

  /// Puts back what the clipboard held before [token]'s write, after
  /// [after], unless the clipboard changed meanwhile.
  Future<void> restoreClipboard(String token, Duration after);
}

class RuntimeModuleTextDelivery implements ModuleTextDelivery {
  const RuntimeModuleTextDelivery();

  @override
  Future<ModuleDelivered> deliver(
    String moduleId,
    ModuleDelivery delivery, {
    required bool paste,
  }) => guardModuleRequest(() async {
    final record = delivery.recordHistory && !delivery.secret;
    final delivered = await clips.deliverText(
      text: delivery.text,
      secret: delivery.secret,
      delivery: paste ? clips.TextDelivery.paste : clips.TextDelivery.copy,
      recordModuleId: record ? moduleId : null,
      recordLabel: record ? delivery.label : null,
      cursorGraphemesFromEnd: delivery.cursorFromEnd,
    );
    return (
      token: delivered.token,
      cursorFromEnd: delivered.cursorGraphemesFromEnd ?? delivery.cursorFromEnd,
    );
  });

  @override
  Future<void> restoreClipboard(String token, Duration after) =>
      guardModuleRequest(
        () =>
            clips.restoreClipboard(token: token, afterMs: after.inMilliseconds),
      );
}
