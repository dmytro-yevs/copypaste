import 'package:flutter/services.dart';

/// Relaunches the native process using the platform-owned shutdown lifecycle.
class ApplicationRestarter {
  const ApplicationRestarter();
  static const _channel = MethodChannel('com.copypaste.app/lifecycle');
  Future<void> restart() => _channel.invokeMethod<void>('restart');
}
