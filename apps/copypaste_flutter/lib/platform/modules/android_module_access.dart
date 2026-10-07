import 'package:flutter/services.dart';
import '../../features/modules/models/module_models.dart';
import '../../features/modules/repository/module_access_repository.dart';

class AndroidModuleAccess implements ModuleAccessRepository {
  const AndroidModuleAccess();
  static const _channel = MethodChannel('com.copypaste.app/sms_modules');
  @override
  Future<SmsModuleAccessState> smsState() => _state('state');
  @override
  Future<SmsModuleAccessState> configureSms() => _state('configure');
  Future<SmsModuleAccessState> _state(String method) async {
    try {
      final value = await _channel.invokeMapMethod<String, Object>(method);
      if (value == null ||
          value['granted'] is! bool ||
          value['adbCommands'] is! String) {
        throw const ModulesException('SMS access could not be verified.');
      }
      return SmsModuleAccessState(
        granted: value['granted'] as bool,
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
