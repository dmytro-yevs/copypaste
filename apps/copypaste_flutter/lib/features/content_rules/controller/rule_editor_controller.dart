import 'dart:async';

import 'package:flutter/foundation.dart';

import '../../../l10n/l10n.dart';
import '../models/content_rule_models.dart';
import '../repository/content_rule_repository.dart';

/// Why Save is unavailable, in the order the footer explains it.
enum RuleSaveBlock { readOnly, nameMissing, nameTooLong, noActions, invalid }

/// Draft state of one rule in the editor. The runtime validates the draft
/// after each pause in editing, so regex errors match every device; nothing
/// is evaluated here. Tests never store their sample.
class RuleEditorController extends ChangeNotifier {
  RuleEditorController({
    required ContentRuleRepository repository,
    required RuleOptions options,
    ContentRule? rule,
    RuleDefinition? definition,
    this.validationDelay = const Duration(milliseconds: 300),
  }) : _repository = repository,
       _options = options,
       _rule = rule,
       _name = rule?.name ?? '',
       _enabled = rule?.enabled ?? true,
       _deviceIds = [...?rule?.deviceIds],
       _definition = rule == null
           ? definition ?? const RuleDefinition()
           : rule.definition ?? const RuleDefinition(),
       _readOnly =
           rule != null &&
           (rule.unsupported || (rule.definition?.needsNewerVersion ?? true)) {
    _initial = _snapshot;
    _initialDefinition = _definition.toJson();
    if (rule != null && !_readOnly) _scheduleValidation(Duration.zero);
  }

  final ContentRuleRepository _repository;
  final RuleOptions _options;
  final ContentRule? _rule;
  final Duration validationDelay;

  String _name;
  bool _enabled;
  List<String> _deviceIds;
  RuleDefinition _definition;
  final bool _readOnly;
  late String _initial;
  late String _initialDefinition;

  List<RuleProblem> _problems = const [];
  bool _validating = false;
  int _validation = 0;
  Timer? _validationTimer;

  bool _saving = false;
  String? _error;

  String _sample = '';
  bool _fullSet = false;
  bool _testing = false;
  int _test = 0;
  RuleTrace? _trace;
  String? _transformed;
  String? _testError;

  bool _disposed = false;

  ContentRule? get rule => _rule;
  RuleOptions get options => _options;
  bool get isNew => _rule == null;
  bool get builtin => _rule?.builtin ?? false;

  /// A rule from a newer CopyPaste, or one whose stored JSON this build
  /// cannot read, is shown but never written back.
  bool get readOnly => _readOnly;
  String get name => _name;
  bool get enabled => _enabled;
  RuleDefinition get definition => _definition;
  List<String> get deviceIds => List.unmodifiable(_deviceIds);
  bool get allDevices => _deviceIds.isEmpty;
  List<RuleProblem> get problems => _problems;
  bool get validating => _validating;
  bool get saving => _saving;
  String? get errorMessage => _error;

  String get sample => _sample;
  bool get fullSet => _fullSet;
  bool get testing => _testing;
  RuleTrace? get trace => _trace;
  String? get transformedOutput => _transformed;
  String? get testError => _testError;

  String get _snapshot =>
      '$_name\u0000$_enabled\u0000${_deviceIds.join(',')}\u0000'
      '${_definition.toJson()}';

  bool get dirty => !_readOnly && _snapshot != _initial;

  RuleSaveBlock? get saveBlock {
    if (_readOnly) return RuleSaveBlock.readOnly;
    final name = _name.trim();
    if (name.isEmpty) return RuleSaveBlock.nameMissing;
    if (name.length > RuleLimits.maxName) return RuleSaveBlock.nameTooLong;
    if (_definition.actions.isEmpty) return RuleSaveBlock.noActions;
    if (_problems.isNotEmpty) return RuleSaveBlock.invalid;
    return null;
  }

  bool get canSave =>
      !_saving && !_validating && saveBlock == null && (dirty || isNew);

  /// Problems of condition [index], such as an unsupported regex.
  List<RuleProblem> conditionProblems(int index) => [
    for (final problem in _problems)
      if (problem.conditionIndex == index) problem,
  ];

  List<RuleProblem> actionProblems(int index) => [
    for (final problem in _problems)
      if (problem.actionIndex == index) problem,
  ];

  /// Problems not tied to one row, such as too many conditions.
  List<RuleProblem> get ruleProblems => [
    for (final problem in _problems)
      if (problem.conditionIndex == null && problem.actionIndex == null)
        problem,
  ];

  void setName(String value) {
    if (_readOnly || value == _name) return;
    _name = value;
    notifyListeners();
  }

  void setEnabled(bool value) {
    if (_readOnly || value == _enabled) return;
    _enabled = value;
    notifyListeners();
  }

  void setAllDevices(bool value) {
    if (_readOnly || value == allDevices) return;
    _deviceIds = value ? [] : [?_options.devices.firstOrNull?.id];
    notifyListeners();
  }

  void setDevices(List<String> ids) {
    if (_readOnly) return;
    _deviceIds = [...ids];
    notifyListeners();
  }

  void setMatchMode(RuleMatchMode mode) =>
      _change(_definition.copyWith(matchMode: mode));

  void setStopProcessing(bool value) =>
      _change(_definition.copyWith(stopProcessing: value));

  bool get canAddCondition =>
      !_readOnly && _definition.conditions.length < RuleLimits.maxConditions;

  void addCondition(RuleConditionType type) {
    if (!canAddCondition) return;
    _change(
      _definition.copyWith(
        conditions: [
          ..._definition.conditions,
          RuleCondition(type: type),
        ],
      ),
    );
  }

  void updateCondition(int index, RuleCondition condition) {
    final conditions = [..._definition.conditions];
    if (index < 0 || index >= conditions.length) return;
    conditions[index] = condition;
    _change(_definition.copyWith(conditions: conditions));
  }

  void removeCondition(int index) {
    final conditions = [..._definition.conditions];
    if (index < 0 || index >= conditions.length) return;
    conditions.removeAt(index);
    _change(_definition.copyWith(conditions: conditions));
  }

  /// Adds [type] unless [RuleDefinition.blockFor] refuses it.
  void addAction(RuleActionType type) {
    if (_readOnly || _definition.blockFor(type) != null) return;
    final action = switch (type) {
      RuleActionType.addTags => RuleAction(
        type: type,
        tagIds: [?_options.tags.firstOrNull?.id],
      ),
      RuleActionType.transform => RuleAction(
        type: type,
        transformId: _options.transforms.firstOrNull?.id ?? '',
      ),
      _ => RuleAction(type: type),
    };
    _change(_definition.copyWith(actions: [..._definition.actions, action]));
  }

  void updateAction(int index, RuleAction action) {
    final actions = [..._definition.actions];
    if (index < 0 || index >= actions.length) return;
    actions[index] = action;
    _change(_definition.copyWith(actions: actions));
  }

  void removeAction(int index) {
    final actions = [..._definition.actions];
    if (index < 0 || index >= actions.length) return;
    actions.removeAt(index);
    _change(_definition.copyWith(actions: actions));
  }

  void _change(RuleDefinition definition) {
    if (_readOnly) return;
    _definition = definition;
    _scheduleValidation(validationDelay);
    notifyListeners();
  }

  void _scheduleValidation(Duration delay) {
    _validationTimer?.cancel();
    _validating = true;
    final generation = ++_validation;
    _validationTimer = Timer(delay, () => unawaited(_validate(generation)));
  }

  Future<void> _validate(int generation) async {
    // An unreachable runtime leaves the draft unmarked; Save reports why.
    List<RuleProblem> problems;
    try {
      problems = await _repository.validate(_definition.toJson());
    } on ContentRuleException {
      problems = const [];
    }
    if (_disposed || generation != _validation) return;
    _problems = problems;
    _validating = false;
    notifyListeners();
  }

  /// The stored text when the definition is unchanged, so editing only the
  /// name or devices neither customizes a built-in definition nor moves the
  /// rule's effective time.
  String get _definitionToSave {
    final json = _definition.toJson();
    final rule = _rule;
    return rule != null && json == _initialDefinition
        ? rule.definitionJson
        : json;
  }

  /// Saves the draft; false keeps the editor open with [errorMessage].
  Future<bool> save() async {
    if (!canSave) return false;
    _saving = true;
    _error = null;
    notifyListeners();
    try {
      await _repository.save(
        id: _rule?.id,
        name: _name.trim(),
        enabled: _enabled,
        definitionJson: _definitionToSave,
        deviceIds: _deviceIds,
      );
      _initial = _snapshot;
      return true;
    } on ContentRuleException catch (error) {
      _error = error.message;
      return false;
    } finally {
      _saving = false;
      notifyListeners();
    }
  }

  void setSample(String value) {
    if (value == _sample) return;
    _sample = value;
    notifyListeners();
  }

  void setFullSet(bool value) {
    if (value == _fullSet) return;
    _fullSet = value;
    notifyListeners();
  }

  bool get canTest =>
      !_testing && _sample.isNotEmpty && (_readOnly || saveBlock == null);

  /// Evaluates the draft (or the stored rule when it is read-only) on the
  /// sample, then shows what its transforms would make of it.
  Future<void> runTest() async {
    if (!canTest) return;
    final generation = ++_test;
    _testing = true;
    _testError = null;
    notifyListeners();
    try {
      final trace = await _repository.test(
        definitionJson: _readOnly ? null : _definition.toJson(),
        ruleId: _rule?.id,
        sampleText: _sample,
        fullSet: _fullSet,
      );
      final transforms = [
        for (final id in trace.transformIds) ?_options.transform(id),
      ];
      final output = trace.secret || trace.skipped
          ? null
          : await _repository.transformOutput(_sample, transforms);
      if (_disposed || generation != _test) return;
      _trace = trace;
      _transformed = output;
    } on ContentRuleException catch (error) {
      if (_disposed || generation != _test) return;
      _trace = null;
      _transformed = null;
      _testError = error.message;
    } on Object {
      if (_disposed || generation != _test) return;
      _testError = AppStrings.current.rulesTestFailed;
    } finally {
      if (!_disposed && generation == _test) {
        _testing = false;
        notifyListeners();
      }
    }
  }

  /// The draft to preview against history; a read-only rule previews as
  /// stored.
  String? get previewDefinitionJson => _readOnly ? null : _definition.toJson();

  @override
  void notifyListeners() {
    if (!_disposed) super.notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    _validationTimer?.cancel();
    super.dispose();
  }
}
