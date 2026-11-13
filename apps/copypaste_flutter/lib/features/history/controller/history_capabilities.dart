import '../../content_rules/models/content_rule_models.dart';
import '../models/history_models.dart';
import '../repository/history_repository.dart';
import 'history_controller.dart';
import 'history_scope.dart';

/// The optional History capabilities a repository may provide: clip editing,
/// the paste queue, transforms and per-clip annotations. A capability is
/// null when the repository does not implement it, and its actions are then
/// not offered anywhere in History.
class HistoryCapabilities {
  HistoryCapabilities(this._scope, this._history);

  final HistoryScope _scope;
  final HistoryController _history;
  final Set<String> _annotationMutations = <String>{};
  int _queueAdds = 0;

  HistoryEditRepository? get edits => switch (_scope.repository) {
    final HistoryEditRepository repository => repository,
    _ => null,
  };

  HistoryQueueRepository? get queue => switch (_scope.repository) {
    final HistoryQueueRepository repository => repository,
    _ => null,
  };

  HistoryTransformRepository? get transforms => switch (_scope.repository) {
    final HistoryTransformRepository repository => repository,
    _ => null,
  };

  HistoryAnnotationsRepository? get annotations => switch (_scope.repository) {
    final HistoryAnnotationsRepository repository => repository,
    _ => null,
  };

  bool get hasPending => _annotationMutations.isNotEmpty || _queueAdds > 0;

  bool isAnnotationPending(String id) => _annotationMutations.contains(id);

  /// The full clip, from the loaded selection when it already holds it.
  Future<HistoryClip> detail(String id) async {
    final selected = _history.selection.selectedClip;
    if (selected?.id == id && selected?.body != null) return selected!;
    return _scope.repository.get(id);
  }

  Future<bool> setHiddenFromAi(HistoryClip clip, bool hidden) =>
      _annotate(clip.id, (repository) async {
        await repository.setHiddenFromAi([clip.id], hidden: hidden);
        final current = clip.annotations;
        return HistoryClipAnnotations(
          expiresAt: current.expiresAt,
          hiddenFromAi: hidden,
          ruleSecret: current.ruleSecret,
          provenance: current.provenance,
          ruleTagIds: current.ruleTagIds,
          keep: current.keep,
        );
      });

  /// Keeps an expiring clip; pinning it does the same.
  Future<bool> keep(HistoryClip clip) =>
      _annotate(clip.id, (repository) => repository.keep(clip.id));

  /// Appends [clipIds] to the queue in the given order; returns the pending
  /// count, or null when the queue refused or failed.
  Future<int?> addToQueue(List<String> clipIds) async {
    final queue = this.queue;
    if (queue == null || clipIds.isEmpty || _scope.disposed) return null;
    _queueAdds++;
    try {
      return await queue.addToQueue(clipIds);
    } catch (_) {
      return null;
    } finally {
      _queueAdds--;
    }
  }

  /// Saves the clip's text as a literal snippet; returns its id or null.
  Future<String?> saveAsSnippet(String clipId) async {
    final edits = this.edits;
    if (edits == null) return null;
    try {
      return (await edits.snippets.saveFromClip(clipId)).id;
    } catch (_) {
      return null;
    }
  }

  /// The rule trace of [clipId] with the names it refers to.
  Future<(RuleTrace, RuleOptions)> explain(String clipId) async {
    final annotations = this.annotations!;
    final trace = annotations.ruleTrace(clipId);
    final options = annotations.ruleOptions().catchError(
      (Object _) => const RuleOptions(),
    );
    return (await trace, await options);
  }

  Future<bool> _annotate(
    String id,
    Future<HistoryClipAnnotations> Function(HistoryAnnotationsRepository) write,
  ) async {
    final repository = annotations;
    if (repository == null ||
        _scope.disposed ||
        !_annotationMutations.add(id)) {
      return false;
    }
    _scope.notify();
    try {
      // Only the annotations change, so the loaded row and detail are
      // patched instead of read again.
      final annotations = await write(repository);
      if (_scope.disposed) return true;
      final row = _history.window.clipById(id);
      if (row != null) {
        _history.window.replace(row.copyWith(annotations: annotations));
      }
      final selected = _history.selection.selectedClip;
      if (selected?.id == id) {
        _history.selection.showDetail(
          id,
          selected!.copyWith(annotations: annotations),
        );
      }
      return true;
    } catch (_) {
      return false;
    } finally {
      _annotationMutations.remove(id);
      if (!_scope.disposed) _scope.notify();
    }
  }
}
