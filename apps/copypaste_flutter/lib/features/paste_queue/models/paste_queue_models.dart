import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

import '../../../platform/desktop/global_shortcut.dart';

enum PasteQueuePhase { empty, active, finished }

enum PasteQueueEntryStatus { pending, pasted, skipped, unavailable }

enum PasteQueueContentKind { text, richText, image, files, link }

enum PasteQueueEndBehavior { stop, loop, keepLast }

enum PasteQueueSeparator {
  newLine,
  blankLine,
  tab,
  space,
  comma,
  commaSpace,
  none,
  custom,
}

/// Why Paste all cannot run on the current pending entries.
enum PasteAllBlock { nothingPending, mixed, severalFiles }

/// Queue settings held by the runtime on this device.
@immutable
class PasteQueueSettings {
  const PasteQueueSettings({
    this.enabled = true,
    this.insertAtStart = false,
    this.endBehavior = PasteQueueEndBehavior.stop,
    this.separator = PasteQueueSeparator.newLine,
    this.customSeparator = '',
    this.plainText = false,
    this.idleStopMinutes = 15,
    this.maxEntries = 200,
    this.moveToNextField = false,
    this.restoreWhenFinished = false,
  });

  static const idleStopOptions = [5, 15, 30, 60, 0];
  static const maxEntriesOptions = [10, 25, 50, 100, 200, 500, 1000];
  static const customSeparatorMaxLength = 32;

  final bool enabled;
  final bool insertAtStart;
  final PasteQueueEndBehavior endBehavior;
  final PasteQueueSeparator separator;
  final String customSeparator;
  final bool plainText;

  /// Zero never stops collecting on its own.
  final int idleStopMinutes;
  final int maxEntries;
  final bool moveToNextField;
  final bool restoreWhenFinished;

  PasteQueueSettings copyWith({
    bool? enabled,
    bool? insertAtStart,
    PasteQueueEndBehavior? endBehavior,
    PasteQueueSeparator? separator,
    String? customSeparator,
    bool? plainText,
    int? idleStopMinutes,
    int? maxEntries,
    bool? moveToNextField,
    bool? restoreWhenFinished,
  }) => PasteQueueSettings(
    enabled: enabled ?? this.enabled,
    insertAtStart: insertAtStart ?? this.insertAtStart,
    endBehavior: endBehavior ?? this.endBehavior,
    separator: separator ?? this.separator,
    customSeparator: customSeparator ?? this.customSeparator,
    plainText: plainText ?? this.plainText,
    idleStopMinutes: idleStopMinutes ?? this.idleStopMinutes,
    maxEntries: maxEntries ?? this.maxEntries,
    moveToNextField: moveToNextField ?? this.moveToNextField,
    restoreWhenFinished: restoreWhenFinished ?? this.restoreWhenFinished,
  );

  @override
  bool operator ==(Object other) =>
      other is PasteQueueSettings &&
      other.enabled == enabled &&
      other.insertAtStart == insertAtStart &&
      other.endBehavior == endBehavior &&
      other.separator == separator &&
      other.customSeparator == customSeparator &&
      other.plainText == plainText &&
      other.idleStopMinutes == idleStopMinutes &&
      other.maxEntries == maxEntries &&
      other.moveToNextField == moveToNextField &&
      other.restoreWhenFinished == restoreWhenFinished;

  @override
  int get hashCode => Object.hash(
    enabled,
    insertAtStart,
    endBehavior,
    separator,
    customSeparator,
    plainText,
    idleStopMinutes,
    maxEntries,
    moveToNextField,
    restoreWhenFinished,
  );
}

@immutable
class PasteQueueEntry {
  const PasteQueueEntry({
    required this.id,
    required this.position,
    required this.status,
    required this.kind,
    this.preview = '',
    this.masked = false,
    this.edited = false,
    this.sourceAppName,
    this.sourceAppIconId,
    this.renderedAt,
  });

  /// The runtime keeps at most this many UTF-8 bytes of a preview.
  static const previewBytes = 1024;

  final String id;
  final String position;
  final PasteQueueEntryStatus status;
  final PasteQueueContentKind kind;

  /// Empty when [masked].
  final String preview;
  final bool masked;
  final bool edited;
  final String? sourceAppName;
  final String? sourceAppIconId;

  /// When a snippet entry was rendered.
  final DateTime? renderedAt;

  bool get pending => status == PasteQueueEntryStatus.pending;

  bool get textual => switch (kind) {
    PasteQueueContentKind.text ||
    PasteQueueContentKind.richText ||
    PasteQueueContentKind.link => true,
    PasteQueueContentKind.image || PasteQueueContentKind.files => false,
  };

  /// A pending text entry whose whole text is in [preview]; a longer one
  /// would lose its tail when edited from the preview.
  bool get editable =>
      textual &&
      !masked &&
      status != PasteQueueEntryStatus.unavailable &&
      utf8.encode(preview).length < previewBytes;

  @override
  bool operator ==(Object other) =>
      other is PasteQueueEntry &&
      other.id == id &&
      other.position == position &&
      other.status == status &&
      other.kind == kind &&
      other.preview == preview &&
      other.masked == masked &&
      other.edited == edited &&
      other.sourceAppName == sourceAppName &&
      other.sourceAppIconId == sourceAppIconId &&
      other.renderedAt == renderedAt;

  @override
  int get hashCode => Object.hash(
    id,
    position,
    status,
    kind,
    preview,
    masked,
    edited,
    sourceAppName,
    sourceAppIconId,
    renderedAt,
  );
}

/// One read of the queue.
@immutable
class PasteQueueSnapshot {
  const PasteQueueSnapshot({
    required this.revision,
    this.collecting = false,
    this.phase = PasteQueuePhase.empty,
    this.full = false,
    this.entries = const [],
    this.staleSince,
    this.undoUntil,
    this.settings = const PasteQueueSettings(),
  });

  static const initial = PasteQueueSnapshot(revision: -1);

  final int revision;
  final bool collecting;
  final PasteQueuePhase phase;
  final bool full;
  final List<PasteQueueEntry> entries;

  /// Set while pending entries are older than a day.
  final DateTime? staleSince;

  /// While set, Undo restores the entries the last Clear removed.
  final DateTime? undoUntil;
  final PasteQueueSettings settings;

  int get pendingCount => entries.where((entry) => entry.pending).length;

  bool get hasFinishedEntries => entries.any(
    (entry) =>
        entry.status == PasteQueueEntryStatus.pasted ||
        entry.status == PasteQueueEntryStatus.skipped,
  );

  PasteAllBlock? get pasteAllBlock {
    final pending = entries.where((entry) => entry.pending).toList();
    if (pending.isEmpty) return PasteAllBlock.nothingPending;
    if (pending.every((entry) => entry.textual)) return null;
    if (pending.every((entry) => entry.kind == PasteQueueContentKind.files)) {
      return pending.length > 1 ? PasteAllBlock.severalFiles : null;
    }
    return PasteAllBlock.mixed;
  }

  /// Keeps everything but the entries, for a queue nobody is viewing.
  PasteQueueSnapshot withoutEntries() => PasteQueueSnapshot(
    revision: revision,
    collecting: collecting,
    phase: phase,
    full: full,
    staleSince: staleSince,
    undoUntil: undoUntil,
    settings: settings,
  );
}

enum PasteQueueTarget { next, all, entry }

/// [clipboard] is followed by a foreground paste and a confirmation;
/// [copyOnly] leaves the paste to the user.
enum PasteQueueDeliveryMode { clipboard, copyOnly }

enum PasteQueueNothing { empty, finished, mixed }

@immutable
class PasteQueueDelivery {
  const PasteQueueDelivery({
    this.token,
    this.entryIds = const [],
    this.unavailable = 0,
    this.finished = false,
    this.nothing,
  });

  final String? token;
  final List<String> entryIds;

  /// Entries skipped because their item no longer exists.
  final int unavailable;
  final bool finished;
  final PasteQueueNothing? nothing;
}

/// What to add to the queue.
sealed class PasteQueueSource {
  const PasteQueueSource();

  const factory PasteQueueSource.clip(String id) = PasteQueueClipSource;

  const factory PasteQueueSource.snippet(
    String id, {
    Map<String, String> values,
  }) = PasteQueueSnippetSource;
}

class PasteQueueClipSource extends PasteQueueSource {
  const PasteQueueClipSource(this.id);

  final String id;
}

class PasteQueueSnippetSource extends PasteQueueSource {
  const PasteQueueSnippetSource(this.id, {this.values = const {}});

  final String id;
  final Map<String, String> values;
}

/// A runtime refusal or failure, with its stable error code.
class PasteQueueException implements Exception {
  const PasteQueueException(this.code, [this.message = '']);

  final String code;
  final String message;

  @override
  String toString() => 'PasteQueueException($code)';
}

/// What Paste next does on this device.
enum PasteNextStyle { pasteIntoApp, clipboardOnly }

/// Queue actions a global shortcut can run.
enum PasteQueueShortcutAction {
  toggleCollecting('copypaste.paste-queue.toggle-collecting'),
  pasteNext('copypaste.paste-queue.paste-next'),
  pasteNextPlain('copypaste.paste-queue.paste-next-plain'),
  pasteAll('copypaste.paste-queue.paste-all'),
  stepBack('copypaste.paste-queue.step-back'),
  skip('copypaste.paste-queue.skip');

  const PasteQueueShortcutAction(this.id);

  /// The registrar id, distinct from every other application shortcut.
  final String id;

  /// Bound by default only on macOS: Ctrl+Alt is AltGr on many Windows and
  /// Linux layouts, so those platforms start unbound.
  DesktopShortcut? defaultFor(TargetPlatform platform) {
    if (platform != TargetPlatform.macOS) return null;
    const modifiers = [
      DesktopShortcutModifier.control,
      DesktopShortcutModifier.meta,
    ];
    return switch (this) {
      toggleCollecting => const DesktopShortcut(
        key: PhysicalKeyboardKey.keyC,
        modifiers: modifiers,
      ),
      pasteNext => const DesktopShortcut(
        key: PhysicalKeyboardKey.keyV,
        modifiers: modifiers,
      ),
      _ => null,
    };
  }
}

/// Device-local queue preferences: shortcuts and delivery style.
@immutable
class PasteQueueDevicePreferences {
  const PasteQueueDevicePreferences({
    this.shortcuts = const {},
    this.pasteNextStyle = PasteNextStyle.pasteIntoApp,
    this.shortcutPromptDismissed = false,
  });

  /// Explicit choices; an action missing here uses its platform default and
  /// a `null` value is deliberately unbound.
  final Map<PasteQueueShortcutAction, DesktopShortcut?> shortcuts;
  final PasteNextStyle pasteNextStyle;
  final bool shortcutPromptDismissed;

  DesktopShortcut? shortcutFor(
    PasteQueueShortcutAction action,
    TargetPlatform platform,
  ) => shortcuts.containsKey(action)
      ? shortcuts[action]
      : action.defaultFor(platform);

  PasteQueueDevicePreferences copyWith({
    Map<PasteQueueShortcutAction, DesktopShortcut?>? shortcuts,
    PasteNextStyle? pasteNextStyle,
    bool? shortcutPromptDismissed,
  }) => PasteQueueDevicePreferences(
    shortcuts: shortcuts ?? this.shortcuts,
    pasteNextStyle: pasteNextStyle ?? this.pasteNextStyle,
    shortcutPromptDismissed:
        shortcutPromptDismissed ?? this.shortcutPromptDismissed,
  );

  PasteQueueDevicePreferences withShortcut(
    PasteQueueShortcutAction action,
    DesktopShortcut? shortcut,
  ) => copyWith(shortcuts: {...shortcuts, action: shortcut});

  @override
  bool operator ==(Object other) =>
      other is PasteQueueDevicePreferences &&
      mapEquals(other.shortcuts, shortcuts) &&
      other.pasteNextStyle == pasteNextStyle &&
      other.shortcutPromptDismissed == shortcutPromptDismissed;

  @override
  int get hashCode => Object.hash(
    Object.hashAllUnordered(
      shortcuts.entries.map((entry) => Object.hash(entry.key, entry.value)),
    ),
    pasteNextStyle,
    shortcutPromptDismissed,
  );
}

/// Where a queue action came from: a global shortcut runs while another
/// application has focus, so its feedback cannot rely on the window.
enum PasteQueueTrigger { shortcut, window }

enum PasteQueueFeedbackKind {
  /// One entry was pasted; [PasteQueueFeedback.position] of `total`.
  pasted,

  /// Entries are on the clipboard for the user to paste.
  copied,
  finished,

  /// Entries were added; `count` of them, `total` now pending.
  added,
  nothingToPaste,
  mixed,
  unavailable,

  /// Synthetic input is blocked; the entry was put on the clipboard only.
  accessibilityMissing,

  /// The session cannot synthesize input; the entry is on the clipboard.
  pasteUnsupported,

  /// The target cannot receive input; the entry stays pending.
  targetElevated,
  noTarget,
  collectingStarted,
  collectingStopped,
  failed,
}

@immutable
class PasteQueueFeedback {
  const PasteQueueFeedback(
    this.kind, {
    required this.trigger,
    this.position = 0,
    this.total = 0,
    this.count = 0,
  });

  final PasteQueueFeedbackKind kind;
  final PasteQueueTrigger trigger;
  final int position;
  final int total;

  /// Entries a notice is about, such as unavailable ones.
  final int count;

  /// Whether this reports a delivery that did not reach the target app.
  bool get problem => switch (kind) {
    PasteQueueFeedbackKind.nothingToPaste ||
    PasteQueueFeedbackKind.mixed ||
    PasteQueueFeedbackKind.unavailable ||
    PasteQueueFeedbackKind.accessibilityMissing ||
    PasteQueueFeedbackKind.pasteUnsupported ||
    PasteQueueFeedbackKind.targetElevated ||
    PasteQueueFeedbackKind.noTarget ||
    PasteQueueFeedbackKind.failed => true,
    _ => false,
  };
}
