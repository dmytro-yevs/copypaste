import 'package:copypaste_flutter/platform/permissions/linux_integration.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('typed capability reports backend and guarantee independently', () {
    final status = LinuxIntegrationStatus.fromMap({
      'session': 'wayland',
      'globalShortcuts': true,
      'remoteDesktop': 'consentRequired',
      'companion': 'unavailable',
      'clipboard': false,
      'quickPaste': false,
      'screenshotProtection': false,
      'startAtLogin': false,
      'uriRegistered': false,
      'capabilities': {
        'globalShortcuts': {
          'state': 'available',
          'backend': 'xdgPortal',
          'guarantee': 'verified',
          'reason': null,
          'permissions': <String>[],
        },
        'quickPaste': {
          'state': 'needs_permission',
          'backend': null,
          'guarantee': 'none',
          'reason': 'Remote Desktop consent is required.',
          'permissions': <String>['remoteDesktop'],
        },
      },
    });

    expect(
      status.capability(LinuxCapabilityKind.globalShortcuts).backend,
      'xdgPortal',
    );
    expect(
      status.capability(LinuxCapabilityKind.globalShortcuts).guarantee,
      LinuxCapabilityGuarantee.verified,
    );
    expect(
      status.capability(LinuxCapabilityKind.quickPaste).state,
      LinuxCapabilityState.needsPermission,
    );
    expect(
      status.capability(LinuxCapabilityKind.quickPaste).permissions,
      <String>['remoteDesktop'],
    );
    expect(
      status.capability(LinuxCapabilityKind.secureStorage).state,
      LinuxCapabilityState.error,
    );
  });

  test('available status requires a concrete backend guarantee', () {
    expect(
      () => LinuxCapabilityStatus.fromMap({
        'state': 'available',
        'backend': null,
        'guarantee': 'none',
        'permissions': <String>[],
      }),
      throwsFormatException,
    );
  });

  test('wire guarantee uses the shared snake case spelling', () {
    final status = LinuxCapabilityStatus.fromMap({
      'state': 'available',
      'backend': 'x11',
      'guarantee': 'best_effort',
      'permissions': <String>[],
    });
    expect(status.guarantee, LinuxCapabilityGuarantee.bestEffort);
  });
}
