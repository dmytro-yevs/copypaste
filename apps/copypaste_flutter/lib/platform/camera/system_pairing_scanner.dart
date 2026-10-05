import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

/// A platform-owned scanner returns one QR result or a user cancellation.
abstract interface class SystemPairingScanner {
  Future<String?> scan();
  Future<void> cancel();

  static SystemPairingScanner? forPlatform() =>
      !kIsWeb && defaultTargetPlatform == TargetPlatform.android
      ? const GooglePairingScanner()
      : null;
}

class GooglePairingScanner implements SystemPairingScanner {
  const GooglePairingScanner({
    MethodChannel channel = const MethodChannel('com.copypaste.app/qr_scanner'),
  }) : _channel = channel;

  final MethodChannel _channel;

  @override
  Future<String?> scan() => _channel.invokeMethod<String>('scan');

  @override
  Future<void> cancel() => _channel.invokeMethod<void>('cancel');
}
