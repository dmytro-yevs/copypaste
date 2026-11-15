import 'dart:async';

import '../../../generated/api/diagnostics.dart' as diagnostics;
import '../models/app_update_models.dart';

/// Records update lifecycle phases in the runtime log as fixed events.
void recordAppUpdatePhase(AppUpdatePhase phase) {
  final event = switch (phase) {
    AppUpdatePhase.checking => diagnostics.AppLogEvent.updateCheckStarted,
    AppUpdatePhase.upToDate => diagnostics.AppLogEvent.updateUpToDate,
    AppUpdatePhase.available => diagnostics.AppLogEvent.updateAvailable,
    AppUpdatePhase.unavailable => diagnostics.AppLogEvent.updateUnavailable,
    AppUpdatePhase.downloading => diagnostics.AppLogEvent.updateDownloading,
    AppUpdatePhase.installing => diagnostics.AppLogEvent.updateInstalling,
    AppUpdatePhase.restartRequired =>
      diagnostics.AppLogEvent.updateRestartRequired,
    AppUpdatePhase.error => diagnostics.AppLogEvent.updateFailed,
    AppUpdatePhase.idle ||
    AppUpdatePhase.preparingModules ||
    AppUpdatePhase.permissionRequired => null,
  };
  if (event == null) return;
  // Logging is best-effort and never delays the update flow.
  unawaited(diagnostics.recordAppEvent(event: event).catchError((Object _) {}));
}
