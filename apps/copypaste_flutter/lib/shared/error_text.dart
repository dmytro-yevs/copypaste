import 'dart:async';
import 'dart:developer' as developer;

import '../generated/api.dart' as runtime;
import '../l10n/l10n.dart';

/// A failure whose [message] is a curated sentence that is safe to show.
abstract interface class UserFacingException implements Exception {
  String get message;
}

/// A runtime refusal: a stable [code] and the runtime's curated [message],
/// which never carries paths or technical detail.
abstract interface class RuntimeFailure implements UserFacingException {
  String get code;
}

/// The one mapping from a failure to the text the user sees.
///
/// Raw technical text (exception names, panics, assertion text, stack
/// traces) never reaches the interface: such failures have no [reason], are
/// logged here for developers, and read as a generic sentence. The native
/// runtime records its own failures in the diagnostics log where they occur.
abstract final class ErrorText {
  /// A specific, localized explanation of [error], or null when none is
  /// known. Use it as the optional line under a title that already says
  /// what failed, so the two never repeat each other.
  static String? reason(Object error) {
    final (code, message) = switch (error) {
      runtime.RuntimeError(:final code, :final message) => (code, message),
      RuntimeFailure(:final code, :final message) => (code, message),
      UserFacingException(:final message) => (null, message),
      TimeoutException() => (_timeout, null),
      _ => (null, null),
    };
    if (code != null) {
      if (_opaqueCodes.contains(code)) return _unknown(error);
      if (_forCode(code) case final known?) return known;
    }
    final text = message?.trim() ?? '';
    if (text.isEmpty) return _unknown(error);
    return text[0].toUpperCase() + text.substring(1);
  }

  /// [reason], or a generic sentence when no specific one is known.
  static String describe(Object error) =>
      reason(error) ?? AppStrings.current.errorUnexpected;

  static const _timeout = 'runtime_timeout';

  /// Codes whose runtime sentence says nothing more than "it failed".
  static const _opaqueCodes = {'internal', 'runtime_internal'};

  static String? _forCode(String code) {
    final l10n = AppStrings.current;
    return switch (code) {
      'daemon_unreachable' => l10n.errorRuntimeUnavailable,
      'daemon_start_failed' ||
      'daemon_spawn_failed' ||
      'daemon_exited_early' => l10n.errorRuntimeStartFailed,
      'runtime_not_initialized' || 'not_ready' => l10n.errorRuntimeStarting,
      _timeout => l10n.errorRuntimeTimeout,
      'protocol_mismatch' => l10n.errorRuntimeMismatch,
      'peer_unreachable' => l10n.errorPeerUnreachable,
      'peer_version' => l10n.errorPeerVersion,
      'rate_limited' => l10n.errorRateLimited,
      _ => null,
    };
  }

  static String? _unknown(Object error) {
    developer.log(
      'Unmapped failure ${error.runtimeType}',
      name: 'copypaste.error',
      error: error,
    );
    return null;
  }
}
