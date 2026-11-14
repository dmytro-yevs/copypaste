import 'dart:async';

import 'package:flutter/foundation.dart';

import '../../../l10n/l10n.dart';
import '../models/content_rule_models.dart';
import '../repository/content_rule_repository.dart';

enum RulePreviewPhase {
  previewing,
  ready,
  applying,
  applied,
  cancelled,
  failed,
}

/// "Preview against history" and "Apply to existing items" for one rule.
///
/// The runtime holds one preview plan (ids and effects, no content) until it
/// is applied or cancelled; [close] cancels it so a dismissed dialog never
/// leaves the plan for the 10-minute deadline.
class RulePreviewController extends ChangeNotifier {
  RulePreviewController({
    required ContentRuleRepository repository,
    this.definitionJson,
    this.ruleId,
    this.fullSet = false,
  }) : _repository = repository;

  final ContentRuleRepository _repository;
  final String? definitionJson;
  final String? ruleId;
  final bool fullSet;

  static int _sequence = 0;

  RulePreviewPhase _phase = RulePreviewPhase.previewing;
  RulePreview? _preview;
  RuleApplyReport? _report;
  RuleApplyProgress? _progress;
  String? _error;
  String? _previewRequest;
  String? _applyRequest;
  StreamSubscription<RuleApplyProgress>? _progressSubscription;
  bool _disposed = false;

  RulePreviewPhase get phase => _phase;
  RulePreview? get preview => _preview;
  RuleApplyReport? get report => _report;
  RuleApplyProgress? get progress => _progress;
  String? get errorMessage => _error;

  bool get canApply =>
      _phase == RulePreviewPhase.ready && (_preview?.counts.matched ?? 0) > 0;

  /// Applying deletes clips, which the confirmation has to say.
  bool get destructive => (_preview?.counts.deleted ?? 0) > 0;

  String _request(String kind) =>
      'content-rule-$kind-${identityHashCode(this)}-${++_sequence}';

  Future<void> start() async {
    final request = _request('preview');
    _previewRequest = request;
    _phase = RulePreviewPhase.previewing;
    _error = null;
    notifyListeners();
    try {
      final preview = await _repository.preview(
        request,
        definitionJson: definitionJson,
        ruleId: ruleId,
        fullSet: fullSet,
      );
      if (_disposed || _phase != RulePreviewPhase.previewing) return;
      _preview = preview;
      _phase = RulePreviewPhase.ready;
    } on Object catch (error) {
      if (_disposed || _phase != RulePreviewPhase.previewing) return;
      _previewRequest = null;
      _fail(error);
    }
    notifyListeners();
  }

  Future<void> apply() async {
    final preview = _preview;
    if (!canApply || preview == null) return;
    final request = _request('apply');
    _applyRequest = request;
    _phase = RulePreviewPhase.applying;
    _progress = RuleApplyProgress(done: 0, total: preview.counts.matched);
    notifyListeners();
    _progressSubscription = _repository.applyProgress(request).listen((
      progress,
    ) {
      _progress = progress;
      notifyListeners();
    }, onError: (Object _) {});
    try {
      _report = await _repository.apply(preview.token, request);
      _phase = _report!.cancelled
          ? RulePreviewPhase.cancelled
          : RulePreviewPhase.applied;
    } on Object catch (error) {
      _fail(error);
    } finally {
      _previewRequest = null;
      _applyRequest = null;
      unawaited(_progressSubscription?.cancel());
      _progressSubscription = null;
    }
    notifyListeners();
  }

  /// Stops a running preview or apply. A stopped apply keeps what it
  /// already changed.
  Future<void> cancel() async {
    switch (_phase) {
      case RulePreviewPhase.previewing:
        _phase = RulePreviewPhase.cancelled;
        notifyListeners();
        await _release();
      case RulePreviewPhase.applying:
        final request = _applyRequest;
        if (request != null) await _quiet(_repository.cancel(request));
      case _:
        break;
    }
  }

  /// Releases the preview plan unless it was applied.
  Future<void> close() => _release();

  Future<void> _release() async {
    final request = _previewRequest;
    _previewRequest = null;
    if (request != null) await _quiet(_repository.cancel(request));
  }

  static Future<void> _quiet(Future<void> work) =>
      work.catchError((Object _) {});

  void _fail(Object error) {
    _phase = RulePreviewPhase.failed;
    _error = error is ContentRuleException
        ? error.message
        : AppStrings.current.rulesPreviewFailed;
  }

  @override
  void notifyListeners() {
    if (!_disposed) super.notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    unawaited(_progressSubscription?.cancel());
    unawaited(_release());
    super.dispose();
  }
}
