import 'dart:async';

import 'package:flutter/foundation.dart';

import '../../../l10n/l10n.dart';
import '../models/content_rule_models.dart';
import '../repository/content_rule_repository.dart';

enum ContentRulesStatus { idle, loading, ready, failed }

/// Where a menu or shortcut moves a user rule.
enum RuleMove { up, down, top, bottom }

/// A request to open the rule editor from another surface, such as History
/// "Create rule from this item". [ruleId] null creates a rule.
class RuleEditorRequest {
  const RuleEditorRequest({this.ruleId, this.definition});

  final String? ruleId;

  /// Prefills a new rule.
  final RuleDefinition? definition;
}

/// Settings → Content rules state. It loads while a view is attached and
/// drops the list and its change subscription when the last view detaches.
class ContentRulesController extends ChangeNotifier {
  ContentRulesController({required ContentRuleRepository repository})
    : _repository = repository;

  final ContentRuleRepository _repository;

  ContentRuleRepository get repository => _repository;

  ContentRulesStatus _status = ContentRulesStatus.idle;
  RuleList _list = const RuleList();
  RuleOptions _options = const RuleOptions();
  String? _error;
  bool _busy = false;
  int _attached = 0;
  int _generation = 0;
  StreamSubscription<void>? _changes;
  bool _disposed = false;

  /// The latest editor request; the attached view opens it and clears it.
  final editorRequest = ValueNotifier<RuleEditorRequest?>(null);

  ContentRulesStatus get status => _status;
  RulePrivacyDefaults get privacyDefaults => _list.privacyDefaults;
  RuleOptions get options => _options;
  String? get errorMessage => _error;
  bool get busy => _busy;

  /// Built-in rules: the privacy guard tier, evaluated before user rules.
  List<ContentRule> get builtins => [
    for (final rule in _list.rules)
      if (rule.builtin && !rule.deleted) rule,
  ];

  /// User rules in evaluation order.
  List<ContentRule> get userRules => [
    for (final rule in _list.rules)
      if (!rule.builtin && !rule.deleted) rule,
  ];

  /// Rules deleted within the last 30 days.
  List<ContentRule> get deletedRules => [
    for (final rule in _list.rules)
      if (rule.deleted) rule,
  ];

  /// Concurrent edits that lost, each restorable.
  List<RuleConflict> get conflicts => _list.conflicts;

  ContentRule? rule(String id) =>
      _list.rules.where((rule) => rule.id == id).firstOrNull;

  void attach() {
    if (_attached++ > 0) return;
    _changes = _repository.changes().listen((_) => unawaited(_reload()));
    unawaited(_reload());
  }

  void detach() {
    if (_attached == 0 || --_attached > 0) return;
    unawaited(_changes?.cancel());
    _changes = null;
    _generation++;
    _list = const RuleList();
    _options = const RuleOptions();
    _status = ContentRulesStatus.idle;
  }

  Future<void> retry() => _reload();

  void requestNewRule({RuleDefinition? definition}) =>
      editorRequest.value = RuleEditorRequest(definition: definition);

  void requestEdit(String id) =>
      editorRequest.value = RuleEditorRequest(ruleId: id);

  Future<void> _reload() async {
    final generation = ++_generation;
    if (_status != ContentRulesStatus.ready) {
      _status = ContentRulesStatus.loading;
      notifyListeners();
    }
    try {
      final options = _repository.options().then<RuleOptions?>(
        (options) => options,
        onError: (Object _) => null,
      );
      final list = await _repository.list();
      final loadedOptions = await options;
      if (_disposed || generation != _generation) return;
      _list = list;
      if (loadedOptions != null) _options = loadedOptions;
      _status = ContentRulesStatus.ready;
      _error = null;
    } on Object catch (error) {
      if (_disposed || generation != _generation) return;
      _status = ContentRulesStatus.failed;
      _error = _message(error);
    }
    notifyListeners();
  }

  Future<bool> _mutate(Future<void> Function() change) async {
    if (_busy) return false;
    _busy = true;
    _error = null;
    notifyListeners();
    try {
      await change();
      return true;
    } on Object catch (error) {
      _error = _message(error);
      return false;
    } finally {
      _busy = false;
      if (!_disposed) {
        notifyListeners();
        unawaited(_reload());
      }
    }
  }

  static String _message(Object error) => error is ContentRuleException
      ? error.message
      : AppStrings.current.rulesLoadFailed;

  Future<bool> setEnabled(String id, bool enabled) =>
      _mutate(() => _repository.setEnabled(id, enabled: enabled));

  /// Moves a rule to Recently deleted; [restore] brings it back.
  Future<bool> delete(String id) => _mutate(() => _repository.delete(id));

  Future<bool> restore(String id) => _mutate(() => _repository.restore(id));

  Future<bool> resetBuiltin(String id) =>
      _mutate(() => _repository.resetBuiltin(id));

  Future<bool> restoreConflict(String conflictId) =>
      _mutate(() => _repository.restoreConflict(conflictId));

  /// Whether [id] can move [move] among the user rules.
  bool canMove(String id, RuleMove move) {
    final rules = userRules;
    final index = rules.indexWhere((rule) => rule.id == id);
    if (index < 0) return false;
    return switch (move) {
      RuleMove.up || RuleMove.top => index > 0,
      RuleMove.down || RuleMove.bottom => index < rules.length - 1,
    };
  }

  /// Moves a user rule without dragging, for menus, keys and screen readers.
  Future<bool> move(String id, RuleMove move) {
    final rules = userRules;
    final index = rules.indexWhere((rule) => rule.id == id);
    if (index < 0 || !canMove(id, move)) return Future.value(false);
    return moveTo(id, switch (move) {
      RuleMove.up => index - 1,
      RuleMove.down => index + 1,
      RuleMove.top => 0,
      RuleMove.bottom => rules.length - 1,
    });
  }

  /// Places user rule [id] at [target] in the list as it is now.
  Future<bool> moveTo(String id, int target) {
    final others = [
      for (final rule in userRules)
        if (rule.id != id) rule,
    ];
    if (others.length == userRules.length) return Future.value(false);
    final at = target.clamp(0, others.length);
    final afterId = at == 0 ? null : others[at - 1].id;
    return _mutate(() => _repository.reorder(id, afterId: afterId));
  }

  Future<List<RuleSkipEvent>> skipLog() => _repository.skipLog();

  @override
  void notifyListeners() {
    if (!_disposed) super.notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    unawaited(_changes?.cancel());
    editorRequest.dispose();
    super.dispose();
  }
}
