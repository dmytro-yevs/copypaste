import 'package:copypaste_flutter/features/update/update.dart';
import 'package:copypaste_flutter/platform/update/app_update_platform.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('com.copypaste.app/app_update');
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;

  tearDown(() => messenger.setMockMethodCallHandler(channel, null));

  test(
    'uses the native Linux package and matching process architecture',
    () async {
      messenger.setMockMethodCallHandler(channel, (call) async {
        expect(call.method, 'availability');
        return {
          'available': true,
          'installationType': 'rpm',
          'architecture': 'aarch64',
        };
      });
      final platform = MethodChannelAppUpdatePlatform(
        channel: channel,
        target: AppUpdateTarget.linux,
        linuxArchitecture: () => LinuxAppUpdateArchitecture.aarch64,
      );

      final availability = await platform.availability();

      expect(availability.available, isTrue);
      expect(
        availability.linuxInstallation?.package,
        LinuxAppUpdatePackage.rpm,
      );
      expect(
        availability.linuxInstallation?.architecture,
        LinuxAppUpdateArchitecture.aarch64,
      );
    },
  );

  test('rejects an unknown or mismatched native Linux capability', () async {
    messenger.setMockMethodCallHandler(
      channel,
      (call) async => {
        'available': true,
        'installationType': 'deb',
        'architecture': 'aarch64',
      },
    );
    final platform = MethodChannelAppUpdatePlatform(
      channel: channel,
      target: AppUpdateTarget.linux,
      linuxArchitecture: () => LinuxAppUpdateArchitecture.x86_64,
    );

    final availability = await platform.availability();

    expect(availability.available, isFalse);
    expect(
      availability.reason,
      'This Linux update does not match the running architecture.',
    );
  });

  test(
    'restores a completed Linux package transaction when the host reports one',
    () async {
      messenger.setMockMethodCallHandler(channel, (call) async {
        expect(call.method, 'restoreInstallation');
        return 'installed';
      });
      final platform = MethodChannelAppUpdatePlatform(
        channel: channel,
        target: AppUpdateTarget.linux,
        linuxArchitecture: () => LinuxAppUpdateArchitecture.x86_64,
      );

      expect(
        await platform.restoreInstallation(),
        AppUpdateInstallResult.installed,
      );
    },
  );
}
