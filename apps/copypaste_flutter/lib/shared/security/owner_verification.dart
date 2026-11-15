import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../l10n/l10n.dart';
import '../../platform/security/device_authenticator.dart';
import '../../platform/security/owner_access.dart';
import '../dialogs/question_dialog.dart';

/// Verifies the device owner through [access], or once through
/// [authenticator] without one. Where the device cannot verify its owner,
/// the user confirms in the app.
Future<bool> verifyDeviceOwner(
  BuildContext context, {
  DeviceAuthenticator? authenticator,
  OwnerAccess? access,
}) async {
  final verifier =
      access ??
      (authenticator == null
          ? null
          : OwnerAccess(authenticator: authenticator));
  final result = verifier == null
      ? OwnerAccessResult.confirmInApp
      : await verifier.request(
          deviceAuthReason(AppStrings.current, DeviceAuthPurpose.unlockSecrets),
        );
  if (result == OwnerAccessResult.granted) return true;
  if (result == OwnerAccessResult.denied || !context.mounted) return false;
  final confirmed = await confirmOwnerInApp(context);
  if (confirmed) verifier?.grant();
  return confirmed;
}
