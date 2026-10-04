import 'package:pub_semver/pub_semver.dart';

import '../models/app_update_models.dart';

abstract interface class AppUpdateRepository {
  Future<AppRelease?> findUpdate({
    required Version currentVersion,
    required AppUpdateTarget target,
  });

  Future<DownloadedAppUpdate> download(
    AppRelease release, {
    required void Function(double progress) onProgress,
  });

  void dispose();
}
