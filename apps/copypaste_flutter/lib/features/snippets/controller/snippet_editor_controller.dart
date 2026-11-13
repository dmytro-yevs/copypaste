import 'dart:async';

import 'package:flutter/foundation.dart';

import '../../../l10n/l10n.dart';
import '../models/snippet_models.dart';
import '../repository/snippet_repository.dart';

/// Why Save is disabled; the editor shows it next to the action.
enum SnippetSaveBlock {
  emptyBody,
  bodyTooLarge,
  titleTooLong,
  abbreviationTooLong,
  abbreviationWhitespace,
  abbreviationTaken,
}

enum SnippetEditorSaveResult {
  saved,

  /// The abbreviation belongs to another snippet; see [SnippetEditorController.takenBy].
  abbreviationTaken,

  /// The renamed abbreviation may be named by other templates; ask whether
  /// to rewrite them and save again with that choice.
  needsReferenceChoice,
  failed,
}

/// The owner of a taken abbreviation, for "Go to snippet".
class SnippetOwner {
  const SnippetOwner({required this.id, required this.title});

  final String id;
  final String title;
}

/// Draft state of the snippet editor. Nothing is written or synced until
/// [save]; lint, uniqueness and preview requests follow edits after a short
/// pause and stop when the editor closes.
class SnippetEditorController extends ChangeNotifier {
  SnippetEditorController({
    required SnippetRepository repository,
    SnippetDetail? initial,
    String? collectionId,
    String abbreviation = '',
    this.inputDelay = const Duration(milliseconds: 300),
  }) : _repository = repository,
       _initial = initial,
       _title = initial?.summary.title ?? '',
       _abbreviation = initial?.summary.abbreviation ?? abbreviation,
       _collectionId = initial?.summary.collectionId ?? collectionId,
       _template = initial?.summary.template ?? false,
       _pinned = initial?.summary.pinned ?? false,
       _concealed = initial?.summary.concealed ?? false,
       _body = initial?.bodyText ?? '',
       _lints = initial?.lints ?? const [] {
    if (_abbreviation.isNotEmpty && initial == null) _scheduleUniqueness();
  }

  final SnippetRepository _repository;
  final SnippetDetail? _initial;

  /// The pause after typing before lint, uniqueness and preview requests.
  final Duration inputDelay;

  String _title;
  String _abbreviation;
  String? _collectionId;
  bool _template;
  bool _pinned;
  bool _concealed;
  String _body;
  List<TemplateLint> _lints;
  SnippetOwner? _takenBy;
  RenderedTemplate? _preview;
  bool _previewWanted = false;
  bool _saving = false;
  String? _error;
  Timer? _lintTimer;
  Timer? _uniqueTimer;
  Timer? _previewTimer;
  int _lintGeneration = 0;
  int _uniqueGeneration = 0;
  int _previewGeneration = 0;
  bool _disposed = false;

  SnippetDetail? get initial => _initial;
  bool get isNew => _initial == null;
  String get title => _title;
  String get abbreviation => _abbreviation;
  String? get collectionId => _collectionId;
  bool get template => _template;
  bool get pinned => _pinned;
  bool get concealed => _concealed;
  String get body => _body;
  List<TemplateLint> get lints => _lints;
  SnippetOwner? get takenBy => _takenBy;
  RenderedTemplate? get preview => _preview;
  bool get saving => _saving;
  String? get errorMessage => _error;

  /// A rich body loses its formatting once the text changes.
  bool get formattingNotice => _initial?.bodyRich != null;

  bool get dirty {
    final initial = _initial;
    if (initial == null) {
      return _body.isNotEmpty ||
          _title.isNotEmpty ||
          _abbreviation.isNotEmpty ||
          _template ||
          _pinned ||
          _concealed;
    }
    final summary = initial.summary;
    return _body != initial.bodyText ||
        _title != summary.title ||
        _abbreviation != (summary.abbreviation ?? '') ||
        _collectionId != summary.collectionId ||
        _template != summary.template ||
        _pinned != summary.pinned ||
        _concealed != summary.concealed;
  }

  SnippetSaveBlock? get saveBlock {
    if (_body.trim().isEmpty) return SnippetSaveBlock.emptyBody;
    if (utf8Length(_body) > SnippetLimits.maxBodyBytes) {
      return SnippetSaveBlock.bodyTooLarge;
    }
    if (_title.length > SnippetLimits.maxTitle) {
      return SnippetSaveBlock.titleTooLong;
    }
    final abbreviation = _abbreviation.trim();
    if (abbreviation.isNotEmpty) {
      switch (abbreviationProblem(abbreviation)) {
        case AbbreviationProblem.tooLong:
          return SnippetSaveBlock.abbreviationTooLong;
        case AbbreviationProblem.whitespace:
          return SnippetSaveBlock.abbreviationWhitespace;
        case null:
          break;
      }
      if (_takenBy != null) return SnippetSaveBlock.abbreviationTaken;
    }
    return null;
  }

  bool get canSave => !_saving && saveBlock == null && (dirty || isNew);

  void setTitle(String value) => _set(() => _title = value);

  void setAbbreviation(String value) {
    if (value == _abbreviation) return;
    _abbreviation = value;
    _takenBy = null;
    _scheduleUniqueness();
    notifyListeners();
  }

  void setCollection(String? id) => _set(() => _collectionId = id);

  void setTemplate(bool value) {
    if (value == _template) return;
    _template = value;
    _scheduleLint();
    _schedulePreview();
    notifyListeners();
  }

  void setPinned(bool value) => _set(() => _pinned = value);

  void setConcealed(bool value) => _set(() => _concealed = value);

  void setBody(String value) {
    if (value == _body) return;
    _body = value;
    _scheduleLint();
    _schedulePreview();
    notifyListeners();
  }

  void _set(VoidCallback change) {
    change();
    notifyListeners();
  }

  void _scheduleLint() {
    _lintTimer?.cancel();
    final generation = ++_lintGeneration;
    _lintTimer = Timer(inputDelay, () async {
      try {
        final lints = await _repository.lint(_body, template: _template);
        if (_disposed || generation != _lintGeneration) return;
        _lints = lints;
        notifyListeners();
      } on Object {
        // Lints are advisory; the next edit asks again.
      }
    });
  }

  void _scheduleUniqueness() {
    _uniqueTimer?.cancel();
    final generation = ++_uniqueGeneration;
    final abbreviation = _abbreviation.trim();
    if (abbreviation.isEmpty || abbreviationProblem(abbreviation) != null) {
      return;
    }
    _uniqueTimer = Timer(inputDelay, () async {
      try {
        final hits = await _repository.search(abbreviation, limit: 8);
        if (_disposed || generation != _uniqueGeneration) return;
        final owner = hits
            .where(
              (hit) =>
                  hit.tier == SnippetMatchTier.exactAbbreviation &&
                  hit.snippet.id != _initial?.id &&
                  !hit.snippet.deleted,
            )
            .firstOrNull
            ?.snippet;
        _takenBy = owner == null
            ? null
            : SnippetOwner(
                id: owner.id,
                title: owner.displayTitle ?? owner.abbreviation ?? '',
              );
        notifyListeners();
      } on Object {
        // Save reports a taken abbreviation in any case.
      }
    });
  }

  /// Keeps the preview current while it is shown; it never reads the
  /// clipboard.
  void setPreviewVisible(bool visible) {
    if (visible == _previewWanted) return;
    _previewWanted = visible;
    if (visible) {
      // Called while the editor builds; the render starts after the frame.
      scheduleMicrotask(() => _schedulePreview(immediately: true));
    } else {
      _previewTimer?.cancel();
    }
  }

  void _schedulePreview({bool immediately = false}) {
    if (!_previewWanted || _disposed) return;
    _previewTimer?.cancel();
    final generation = ++_previewGeneration;
    Future<void> run() async {
      if (_body.isEmpty) {
        _preview = const RenderedTemplate(text: '');
        notifyListeners();
        return;
      }
      try {
        final rendered = _template
            ? await _repository.render(body: _body)
            : RenderedTemplate(text: _body);
        if (_disposed || generation != _previewGeneration) return;
        _preview = rendered;
        notifyListeners();
      } on Object {
        // The preview keeps its last result.
      }
    }

    if (immediately) {
      unawaited(run());
    } else {
      _previewTimer = Timer(inputDelay, () => unawaited(run()));
    }
  }

  bool get _abbreviationRenamed {
    final previous = _initial?.summary.abbreviation ?? '';
    final next = _abbreviation.trim();
    return !isNew && previous.isNotEmpty && next.isNotEmpty && next != previous;
  }

  /// Saves the draft. A renamed abbreviation asks first whether to rewrite
  /// the templates that name it ([updateReferences]).
  Future<SnippetEditorSaveResult> save({bool? updateReferences}) async {
    if (_saving || saveBlock != null) return SnippetEditorSaveResult.failed;
    final rename = _abbreviationRenamed;
    if (rename && updateReferences == null) {
      return SnippetEditorSaveResult.needsReferenceChoice;
    }
    _saving = true;
    _error = null;
    notifyListeners();
    try {
      final initial = _initial;
      final abbreviation = _abbreviation.trim();
      final outcome = await _repository.save(
        SnippetDraft(
          id: initial?.id,
          collectionId: _collectionId,
          title: _title.trim(),
          abbreviation: rename
              ? initial!.summary.abbreviation
              : (abbreviation.isEmpty ? null : abbreviation),
          bodyText: normalizeLineEndings(_body),
          bodyRich: initial != null && _body == initial.bodyText
              ? initial.bodyRich
              : null,
          template: _template,
          pinned: _pinned,
          concealed: _concealed,
          baseRevision: initial?.revision,
        ),
      );
      switch (outcome) {
        case SnippetAbbreviationTaken(:final snippetId, :final title):
          _takenBy = SnippetOwner(id: snippetId, title: title);
          return SnippetEditorSaveResult.abbreviationTaken;
        case SnippetSaved(:final detail):
          if (rename) {
            await _repository.renameAbbreviation(
              detail.id,
              abbreviation,
              updateReferences: updateReferences!,
            );
          }
          return SnippetEditorSaveResult.saved;
      }
    } on Object catch (error) {
      _error = error is SnippetException
          ? error.message
          : AppStrings.current.snippetsSaveFailed;
      return SnippetEditorSaveResult.failed;
    } finally {
      _saving = false;
      notifyListeners();
    }
  }

  /// Moves the edited snippet to Recently Deleted.
  Future<bool> delete() async {
    final id = _initial?.id;
    if (id == null || _saving) return false;
    _saving = true;
    notifyListeners();
    try {
      await _repository.delete([id]);
      return true;
    } on Object catch (error) {
      _error = error is SnippetException
          ? error.message
          : AppStrings.current.snippetsSaveFailed;
      return false;
    } finally {
      _saving = false;
      notifyListeners();
    }
  }

  @override
  void notifyListeners() {
    if (!_disposed) super.notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    _lintTimer?.cancel();
    _uniqueTimer?.cancel();
    _previewTimer?.cancel();
    super.dispose();
  }
}
