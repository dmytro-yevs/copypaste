import 'dart:async';

import 'package:copypaste_flutter/features/history/repository/clip_edit_repository.dart';
import 'package:copypaste_flutter/generated/api.dart' as runtime;
import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:copypaste_flutter/shared/error_text.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  setUp(() => AppStrings.useLocale(AppLocalization.fallback));

  test('technical failures never reach the user', () {
    for (final error in [
      PanicException('assertion `left == right` failed left: 0 right: 4'),
      StateError('bad state'),
      Exception('socket closed'),
      const runtime.RuntimeError(
        code: 'internal',
        message: 'CopyPaste could not complete the request.',
      ),
    ]) {
      expect(ErrorText.reason(error), isNull, reason: '$error');
      expect(ErrorText.describe(error), 'Something went wrong. Try again.');
    }
  });

  test('known runtime codes read as localized sentences', () {
    expect(
      ErrorText.describe(
        const runtime.RuntimeError(
          code: 'daemon_unreachable',
          message: 'CopyPaste is not running.',
        ),
      ),
      "CopyPaste isn't running in the background. Restart the app.",
    );
    expect(
      ErrorText.describe(TimeoutException('slow')),
      "CopyPaste didn't respond in time. Try again.",
    );
    AppStrings.useLocale(const Locale('uk'));
    addTearDown(() => AppStrings.useLocale(AppLocalization.fallback));
    expect(
      ErrorText.describe(const ClipEditException('rate_limited', 'slow down')),
      'Забагато спроб. Зачекайте трохи й спробуйте ще раз.',
    );
  });

  test('curated runtime refusals keep their specific reason', () {
    expect(
      ErrorText.reason(
        const ClipEditException(
          'invalid_request',
          'This abbreviation is already used by another snippet.',
        ),
      ),
      'This abbreviation is already used by another snippet.',
    );
    expect(
      ErrorText.reason(
        const runtime.RuntimeError(
          code: 'not_found',
          message: 'item not found',
        ),
      ),
      'Item not found',
    );
  });
}
