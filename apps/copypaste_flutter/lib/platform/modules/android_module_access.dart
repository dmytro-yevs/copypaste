import 'package:flutter/services.dart';
import '../android/android_capture_setup_gateway.dart';
import '../../features/modules/models/module_models.dart';
import '../../features/modules/repository/module_access_repository.dart';

class AndroidModuleAccess implements ModuleAccessRepository {
  const AndroidModuleAccess({
    MethodChannel channel = const MethodChannel(
      'com.copypaste.app/sms_modules',
    ),
    AndroidCaptureSetupGateway? setup,
  }) : _channel = channel,
       _setup = setup;
  final MethodChannel _channel;
  final AndroidCaptureSetupGateway? _setup;
  static final _defaultSetup = MethodChannelAndroidCaptureSetupGateway();

  @override
  Future<bool> openShizuku() => (_setup ?? _defaultSetup).openShizuku();

  @override
  Future<SmsModuleAccessState> requestSmsNotifications() async {
    await (_setup ?? _defaultSetup).requestNotifications();
    return smsState();
  }

  @override
  Future<SmsModuleAccessState> smsState() => _state('state');
  @override
  Future<SmsModuleAccessState> configureSms() => _state('configure');
  Future<SmsModuleAccessState> _state(String method) async {
    try {
      final value = await _channel.invokeMapMethod<String, Object>(method);
      final shizuku = value?['shizuku'];
      if (value == null ||
          value['smsGranted'] is! bool ||
          value['notificationGranted'] is! bool ||
          value['adbCommands'] is! String ||
          shizuku is! Map ||
          [
            'supported',
            'installed',
            'running',
            'permission',
          ].any((key) => shizuku[key] is! bool)) {
        throw const ModulesException('SMS access could not be verified.');
      }
      return SmsModuleAccessState(
        smsGranted: value['smsGranted'] as bool,
        notificationGranted: value['notificationGranted'] as bool,
        shizuku: AndroidShizukuState(
          supported: shizuku['supported'] as bool,
          installed: shizuku['installed'] as bool,
          running: shizuku['running'] as bool,
          permission: shizuku['permission'] as bool,
        ),
        adbCommands: value['adbCommands'] as String,
      );
    } on PlatformException catch (error) {
      throw ModulesException(
        error.message ?? 'SMS access could not be configured.',
      );
    }
  }

  @override
  Future<bool> synchronize() async =>
      await _channel.invokeMethod<bool>('synchronize') ?? false;
}
