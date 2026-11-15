import '../../../l10n/l10n.dart';
import '../controller/paste_queue_shortcuts_controller.dart';
import '../models/paste_queue_models.dart';

/// User-facing text for queue states, feedback and settings choices.
extension PasteQueueMessages on AppLocalizations {
  /// What a screen reader announces and a notification says.
  String pasteQueueFeedback(PasteQueueFeedback feedback) {
    final all = feedback.count > 1;
    return switch (feedback.kind) {
      PasteQueueFeedbackKind.pasted =>
        all
            ? pasteQueuePastedAll(feedback.count)
            : pasteQueuePasted(feedback.position, feedback.total),
      PasteQueueFeedbackKind.copied =>
        all
            ? pasteQueueCopiedAll(feedback.count)
            : pasteQueueCopied(feedback.position, feedback.total),
      PasteQueueFeedbackKind.finished => pasteQueueFinished,
      PasteQueueFeedbackKind.added => pasteQueueAdded(feedback.total),
      PasteQueueFeedbackKind.nothingToPaste => pasteQueueNothingToPaste,
      PasteQueueFeedbackKind.mixed => pasteQueueMixed,
      PasteQueueFeedbackKind.unavailable => pasteQueueUnavailable(
        feedback.count,
      ),
      PasteQueueFeedbackKind.accessibilityMissing =>
        pasteQueueAccessibilityMessage,
      PasteQueueFeedbackKind.pasteUnsupported => pasteQueuePasteUnsupported,
      PasteQueueFeedbackKind.targetElevated => pasteQueueTargetElevated,
      PasteQueueFeedbackKind.noTarget => pasteQueueNoTarget,
      PasteQueueFeedbackKind.collectingStarted => pasteQueueCollectingStarted,
      PasteQueueFeedbackKind.collectingStopped => pasteQueueCollectingStopped,
      PasteQueueFeedbackKind.failed => pasteQueueFailed,
    };
  }

  String pasteQueueBlock(PasteAllBlock block) => switch (block) {
    PasteAllBlock.nothingPending => pasteQueueNothingToPaste,
    PasteAllBlock.mixed => pasteQueueMixed,
    PasteAllBlock.severalFiles => pasteQueueSeveralFiles,
  };

  String pasteQueueShortcutAction(PasteQueueShortcutAction action) =>
      switch (action) {
        PasteQueueShortcutAction.toggleCollecting =>
          pasteQueueShortcutToggleCollecting,
        PasteQueueShortcutAction.pasteNext => pasteQueuePasteNext,
        PasteQueueShortcutAction.pasteNextPlain =>
          pasteQueueShortcutPasteNextPlain,
        PasteQueueShortcutAction.pasteAll => pasteQueuePasteAll,
        PasteQueueShortcutAction.stepBack => pasteQueueStepBack,
        PasteQueueShortcutAction.skip => pasteQueueSkip,
      };

  String pasteQueueShortcutProblem(PasteQueueShortcutProblem problem) =>
      switch (problem) {
        PasteQueueShortcutProblem.heldByApp => pasteQueueShortcutHeldByApp,
        PasteQueueShortcutProblem.inUse => pasteQueueShortcutInUse,
        PasteQueueShortcutProblem.rejected => pasteQueueShortcutRejected,
        PasteQueueShortcutProblem.altGr => pasteQueueShortcutAltGr,
        PasteQueueShortcutProblem.unavailable => pasteQueueShortcutUnavailable,
        PasteQueueShortcutProblem.failed => pasteQueueShortcutFailed,
      };

  String pasteQueueSeparatorName(PasteQueueSeparator separator) =>
      switch (separator) {
        PasteQueueSeparator.newLine => pasteQueueSeparatorNewLine,
        PasteQueueSeparator.blankLine => pasteQueueSeparatorBlankLine,
        PasteQueueSeparator.tab => pasteQueueSeparatorTab,
        PasteQueueSeparator.space => pasteQueueSeparatorSpace,
        PasteQueueSeparator.comma => pasteQueueSeparatorComma,
        PasteQueueSeparator.commaSpace => pasteQueueSeparatorCommaSpace,
        PasteQueueSeparator.none => pasteQueueSeparatorNone,
        PasteQueueSeparator.custom => pasteQueueSeparatorCustom,
      };

  String pasteQueueEndBehaviorName(PasteQueueEndBehavior behavior) =>
      switch (behavior) {
        PasteQueueEndBehavior.stop => pasteQueueEndStop,
        PasteQueueEndBehavior.loop => pasteQueueEndLoop,
        PasteQueueEndBehavior.keepLast => pasteQueueEndKeepLast,
      };

  String pasteQueueIdleStopName(int minutes) =>
      minutes == 0 ? pasteQueueIdleNever : pasteQueueIdleMinutes(minutes);

  String pasteQueueStyleName(PasteNextStyle style) => switch (style) {
    PasteNextStyle.pasteIntoApp => pasteQueuePasteIntoApp,
    PasteNextStyle.clipboardOnly => pasteQueueClipboardOnly,
  };

  /// The status word of a row; status is never shown by color alone.
  String? pasteQueueStatusName(PasteQueueEntryStatus status) =>
      switch (status) {
        PasteQueueEntryStatus.pending => null,
        PasteQueueEntryStatus.pasted => pasteQueueStatusPasted,
        PasteQueueEntryStatus.skipped => pasteQueueStatusSkipped,
        PasteQueueEntryStatus.unavailable => pasteQueueStatusUnavailable,
      };

  /// What a row shows when its preview is not text.
  String pasteQueueContent(PasteQueueEntry entry) {
    if (entry.masked) return pasteQueueSecret;
    return switch (entry.kind) {
      PasteQueueContentKind.image => pasteQueueKindImage,
      PasteQueueContentKind.files =>
        entry.preview.isEmpty ? pasteQueueKindFiles : entry.preview,
      _ => entry.preview,
    };
  }
}
