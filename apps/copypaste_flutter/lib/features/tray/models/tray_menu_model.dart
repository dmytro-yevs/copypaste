import 'package:flutter/widgets.dart' show Characters, IconData;
import 'package:intl/intl.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart' show LucideIcons;

import '../../../l10n/app_localizations.dart';
import '../../../platform/capture/capture_control_state.dart';

/// A tray or menu-bar command that carries no data.
enum TrayCommand {
  openApp,
  openQuickPaste,
  newSnippet,
  copyLatestFromOtherDevice,
  captureNow,
  toggleCapture,
  openPasteQueue,
  openContentRules,
  openSettings,
  quit,
}

/// What a tray menu item does when chosen.
sealed class TrayAction {
  const TrayAction();
}

final class TrayCommandAction extends TrayAction {
  const TrayCommandAction(this.command);

  final TrayCommand command;

  @override
  bool operator ==(Object other) =>
      other is TrayCommandAction && other.command == command;

  @override
  int get hashCode => command.hashCode;
}

/// Pause capture for [duration], or until resumed when it is null.
final class TrayPauseAction extends TrayAction {
  const TrayPauseAction(this.duration);

  final Duration? duration;

  @override
  bool operator ==(Object other) =>
      other is TrayPauseAction && other.duration == duration;

  @override
  int get hashCode => duration.hashCode;
}

/// Copy one clip from the Recent section.
final class TrayCopyClipAction extends TrayAction {
  const TrayCopyClipAction(this.clipId);

  final String clipId;

  @override
  bool operator ==(Object other) =>
      other is TrayCopyClipAction && other.clipId == clipId;

  @override
  int get hashCode => clipId.hashCode;
}

/// How much of a clip the Recent section may show outside the encrypted
/// store: the same levels as the Android widget.
enum TrayPreviewLevel { full, titles, hidden }

enum TrayClipKind { text, image, file, other }

/// The part of a clip the Recent section needs.
class TrayClip {
  const TrayClip({
    required this.id,
    required this.kind,
    required this.masked,
    required this.capturedAt,
    this.preview,
    this.sourceApp,
  });

  final String id;
  final TrayClipKind kind;

  /// Secret, or masked by a content rule; never previewed.
  final bool masked;
  final DateTime capturedAt;

  /// Text content, when the clip has any.
  final String? preview;
  final String? sourceApp;

  @override
  bool operator ==(Object other) =>
      other is TrayClip &&
      other.id == id &&
      other.kind == kind &&
      other.masked == masked &&
      other.capturedAt == capturedAt &&
      other.preview == preview &&
      other.sourceApp == sourceApp;

  @override
  int get hashCode =>
      Object.hash(id, kind, masked, capturedAt, preview, sourceApp);
}

enum TrayLoadStatus { idle, loading, ready, failed }

class TrayQueueStatus {
  const TrayQueueStatus({required this.count, required this.collecting});

  final int count;
  final bool collecting;

  bool get visible => count > 0 || collecting;

  @override
  bool operator ==(Object other) =>
      other is TrayQueueStatus &&
      other.count == count &&
      other.collecting == collecting;

  @override
  int get hashCode => Object.hash(count, collecting);
}

class TraySkipEntry {
  const TraySkipEntry({
    required this.at,
    required this.ruleName,
    this.sourceApp,
  });

  final DateTime at;
  final String ruleName;
  final String? sourceApp;

  @override
  bool operator ==(Object other) =>
      other is TraySkipEntry &&
      other.at == at &&
      other.ruleName == ruleName &&
      other.sourceApp == sourceApp;

  @override
  int get hashCode => Object.hash(at, ruleName, sourceApp);
}

const _unset = Object();

/// Everything the tray menu shows. The window controller owns capture; the
/// tray controller owns the lazily loaded sections and feature slots.
class TrayMenuState {
  const TrayMenuState({
    this.capture = CaptureControlState.unavailable,
    this.pausedUntil,
    this.quickPasteShortcut,
    this.newSnippetAvailable = false,
    this.clipsAvailable = false,
    this.recentStatus = TrayLoadStatus.idle,
    this.recent = const [],
    this.previewLevel = TrayPreviewLevel.full,
    this.secretsCopyable = false,
    this.latestFromOtherDevice,
    this.queue,
    this.pasteQueueAvailable = false,
    this.skipped = const [],
    this.contentRulesAvailable = false,
  });

  final CaptureControlState capture;
  final DateTime? pausedUntil;

  /// The registered Quick Paste shortcut as the platform writes it.
  final String? quickPasteShortcut;
  final bool newSnippetAvailable;

  /// Whether a controller serves Recent, Copy latest and Capture now; until
  /// one attaches, those rows are left out.
  final bool clipsAvailable;
  final TrayLoadStatus recentStatus;
  final List<TrayClip> recent;
  final TrayPreviewLevel previewLevel;

  /// Whether device authentication can unlock a secret copy here.
  final bool secretsCopyable;
  final TrayClip? latestFromOtherDevice;
  final TrayQueueStatus? queue;
  final bool pasteQueueAvailable;
  final List<TraySkipEntry> skipped;
  final bool contentRulesAvailable;

  TrayMenuState copyWith({
    CaptureControlState? capture,
    Object? pausedUntil = _unset,
    Object? quickPasteShortcut = _unset,
    bool? newSnippetAvailable,
    bool? clipsAvailable,
    TrayLoadStatus? recentStatus,
    List<TrayClip>? recent,
    TrayPreviewLevel? previewLevel,
    bool? secretsCopyable,
    Object? latestFromOtherDevice = _unset,
    Object? queue = _unset,
    bool? pasteQueueAvailable,
    List<TraySkipEntry>? skipped,
    bool? contentRulesAvailable,
  }) => TrayMenuState(
    capture: capture ?? this.capture,
    pausedUntil: identical(pausedUntil, _unset)
        ? this.pausedUntil
        : pausedUntil as DateTime?,
    quickPasteShortcut: identical(quickPasteShortcut, _unset)
        ? this.quickPasteShortcut
        : quickPasteShortcut as String?,
    newSnippetAvailable: newSnippetAvailable ?? this.newSnippetAvailable,
    clipsAvailable: clipsAvailable ?? this.clipsAvailable,
    recentStatus: recentStatus ?? this.recentStatus,
    recent: recent ?? this.recent,
    previewLevel: previewLevel ?? this.previewLevel,
    secretsCopyable: secretsCopyable ?? this.secretsCopyable,
    latestFromOtherDevice: identical(latestFromOtherDevice, _unset)
        ? this.latestFromOtherDevice
        : latestFromOtherDevice as TrayClip?,
    queue: identical(queue, _unset) ? this.queue : queue as TrayQueueStatus?,
    pasteQueueAvailable: pasteQueueAvailable ?? this.pasteQueueAvailable,
    skipped: skipped ?? this.skipped,
    contentRulesAvailable: contentRulesAvailable ?? this.contentRulesAvailable,
  );
}

/// Receives the menu's lifecycle and every action the window owner does not
/// handle itself.
abstract interface class TrayMenuDelegate {
  void trayMenuOpened();

  void trayMenuClosed();

  Future<void> trayActionSelected(TrayAction action);
}

/// The owner of the native tray menu that feature controllers feed.
abstract interface class TrayMenuSurface {
  TrayMenuState get trayState;

  Future<void> updateTrayMenu(TrayMenuState Function(TrayMenuState) update);

  void setTrayMenuDelegate(TrayMenuDelegate? delegate);

  Future<void> showFromTrayOrDock();
}

/// One native menu row. [key] is stable across rebuilds, so a host can reuse
/// its native item and change only what differs.
sealed class TrayMenuEntry {
  const TrayMenuEntry(this.key);

  final String key;
}

final class TrayMenuItem extends TrayMenuEntry {
  const TrayMenuItem(
    super.key, {
    required this.label,
    this.icon,
    this.action,
    this.enabled = true,
  });

  final String label;
  final IconData? icon;

  /// Null for a status row that only informs.
  final TrayAction? action;
  final bool enabled;

  bool get selectable => enabled && action != null;

  @override
  bool operator ==(Object other) =>
      other is TrayMenuItem &&
      other.key == key &&
      other.label == label &&
      other.icon == icon &&
      other.action == action &&
      other.enabled == enabled;

  @override
  int get hashCode => Object.hash(key, label, icon, action, enabled);
}

final class TrayMenuSubmenu extends TrayMenuEntry {
  const TrayMenuSubmenu(
    super.key, {
    required this.label,
    required this.children,
    this.icon,
    this.enabled = true,
  });

  final String label;
  final IconData? icon;
  final List<TrayMenuEntry> children;
  final bool enabled;
}

final class TrayMenuSeparator extends TrayMenuEntry {
  const TrayMenuSeparator(super.key);

  @override
  bool operator ==(Object other) =>
      other is TrayMenuSeparator && other.key == key;

  @override
  int get hashCode => key.hashCode;
}

/// Stable entry keys that hosts and tests refer to.
abstract final class TrayMenuKeys {
  static const openApp = 'open-app';
  static const openQuickPaste = 'open-quick-paste';
  static const newSnippet = 'new-snippet';
  static const recent = 'recent';
  static const copyLatest = 'copy-latest';
  static const captureNow = 'capture-now';
  static const toggleCapture = 'toggle-capture';
  static const pauseFor = 'pause-for';
  static const pausedUntil = 'paused-until';
  static const pasteQueue = 'paste-queue';
  static const skipped = 'recently-skipped';
  static const settings = 'settings';
  static const quit = 'quit';

  /// Recent rows are keyed by position, so their native items are reused.
  static String recentSlot(int index) => 'recent-$index';
  static String skippedSlot(int index) => 'skipped-$index';
}

/// The most Recent rows the menu shows.
const trayRecentMaxItems = 10;

/// The most skipped captures the menu lists.
const trayMaxSkipEntries = 10;

/// Preview characters per Recent row, under the widget's 120-character cap.
const trayPreviewMaxCharacters = 60;

/// Builds the whole tray and menu-bar menu, identically on every desktop.
List<TrayMenuEntry> buildTrayMenu(TrayMenuState state, AppLocalizations l10n) {
  final time = _timeFormat(l10n.localeName);
  final queue = state.queue;
  final latest = state.latestFromOtherDevice;
  final shortcut = state.quickPasteShortcut;
  return [
    TrayMenuItem(
      TrayMenuKeys.openApp,
      label: l10n.trayOpenApp,
      icon: LucideIcons.appWindow,
      action: const TrayCommandAction(TrayCommand.openApp),
    ),
    TrayMenuItem(
      TrayMenuKeys.openQuickPaste,
      label: shortcut == null
          ? l10n.trayOpenQuickPaste
          : l10n.trayOpenQuickPasteWithShortcut(shortcut),
      icon: LucideIcons.clipboardList,
      action: const TrayCommandAction(TrayCommand.openQuickPaste),
    ),
    TrayMenuItem(
      TrayMenuKeys.newSnippet,
      label: l10n.trayNewSnippet,
      icon: LucideIcons.notebookPen,
      action: const TrayCommandAction(TrayCommand.newSnippet),
      enabled: state.newSnippetAvailable,
    ),
    if (state.clipsAvailable) ...[
      const TrayMenuSeparator('separator-open'),
      TrayMenuSubmenu(
        TrayMenuKeys.recent,
        label: l10n.trayRecent,
        icon: LucideIcons.history,
        children: _recentEntries(state, l10n, time),
      ),
      TrayMenuItem(
        TrayMenuKeys.copyLatest,
        label: l10n.trayCopyLatestFromOtherDevice,
        icon: LucideIcons.monitorSmartphone,
        action: const TrayCommandAction(TrayCommand.copyLatestFromOtherDevice),
        enabled: latest != null && (!latest.masked || state.secretsCopyable),
      ),
      TrayMenuItem(
        TrayMenuKeys.captureNow,
        label: l10n.trayCaptureNow,
        icon: LucideIcons.clipboardPlus,
        action: const TrayCommandAction(TrayCommand.captureNow),
        enabled: state.capture != CaptureControlState.unavailable,
      ),
    ],
    const TrayMenuSeparator('separator-capture'),
    TrayMenuItem(
      TrayMenuKeys.toggleCapture,
      label: state.capture.actionLabel,
      icon: state.capture.canResume ? LucideIcons.play : LucideIcons.pause,
      action: const TrayCommandAction(TrayCommand.toggleCapture),
      enabled: state.capture != CaptureControlState.unavailable,
    ),
    if (state.pausedUntil case final until? when state.capture.canResume)
      TrayMenuItem(
        TrayMenuKeys.pausedUntil,
        label: l10n.trayPausedUntil(time(until)),
        enabled: false,
      ),
    TrayMenuSubmenu(
      TrayMenuKeys.pauseFor,
      label: l10n.trayPauseFor,
      icon: LucideIcons.timer,
      enabled: state.capture == CaptureControlState.running,
      children: [
        TrayMenuItem(
          'pause-15-minutes',
          label: l10n.trayPauseFor15Minutes,
          action: const TrayPauseAction(Duration(minutes: 15)),
        ),
        TrayMenuItem(
          'pause-1-hour',
          label: l10n.trayPauseForOneHour,
          action: const TrayPauseAction(Duration(hours: 1)),
        ),
        TrayMenuItem(
          'pause-until-resumed',
          label: l10n.trayPauseUntilResumed,
          action: const TrayPauseAction(null),
        ),
      ],
    ),
    if ((queue != null && queue.visible) || state.skipped.isNotEmpty)
      const TrayMenuSeparator('separator-activity'),
    if (queue != null && queue.visible)
      TrayMenuItem(
        TrayMenuKeys.pasteQueue,
        label: queue.collecting
            ? l10n.trayPasteQueueCollecting(queue.count)
            : l10n.trayPasteQueue(queue.count),
        icon: LucideIcons.listOrdered,
        action: const TrayCommandAction(TrayCommand.openPasteQueue),
        enabled: state.pasteQueueAvailable,
      ),
    if (state.skipped.isNotEmpty)
      TrayMenuSubmenu(
        TrayMenuKeys.skipped,
        label: l10n.trayRecentlySkipped(state.skipped.length),
        icon: LucideIcons.shieldOff,
        children: [
          for (final (index, entry)
              in state.skipped.take(trayMaxSkipEntries).indexed)
            TrayMenuItem(
              TrayMenuKeys.skippedSlot(index),
              label: _joined([time(entry.at), entry.sourceApp, entry.ruleName]),
              enabled: false,
            ),
          const TrayMenuSeparator('separator-skipped'),
          TrayMenuItem(
            'open-content-rules',
            label: l10n.trayOpenContentRules,
            action: const TrayCommandAction(TrayCommand.openContentRules),
            enabled: state.contentRulesAvailable,
          ),
        ],
      ),
    const TrayMenuSeparator('separator-app'),
    TrayMenuItem(
      TrayMenuKeys.settings,
      label: l10n.platformTraySettings,
      icon: LucideIcons.settings,
      action: const TrayCommandAction(TrayCommand.openSettings),
    ),
    TrayMenuItem(
      TrayMenuKeys.quit,
      label: l10n.platformTrayQuit,
      icon: LucideIcons.power,
      action: const TrayCommandAction(TrayCommand.quit),
    ),
  ];
}

List<TrayMenuEntry> _recentEntries(
  TrayMenuState state,
  AppLocalizations l10n,
  String Function(DateTime) time,
) {
  TrayMenuItem status(String label) =>
      TrayMenuItem(TrayMenuKeys.recentSlot(0), label: label, enabled: false);
  switch (state.recentStatus) {
    case TrayLoadStatus.idle:
    case TrayLoadStatus.loading:
      return [status(l10n.trayRecentLoading)];
    case TrayLoadStatus.failed:
      return [status(l10n.trayRecentFailed)];
    case TrayLoadStatus.ready:
      break;
  }
  final clips = state.recent.take(trayRecentMaxItems).toList();
  if (clips.isEmpty) return [status(l10n.trayRecentEmpty)];
  if (state.previewLevel == TrayPreviewLevel.hidden) {
    return [status(l10n.trayRecentHidden(clips.length))];
  }
  return [
    for (final (index, clip) in clips.indexed)
      TrayMenuItem(
        TrayMenuKeys.recentSlot(index),
        label: trayClipLabel(clip, state.previewLevel, l10n, time),
        icon: clip.masked ? LucideIcons.lock : null,
        action: TrayCopyClipAction(clip.id),
        enabled: !clip.masked || state.secretsCopyable,
      ),
  ];
}

/// One Recent row: a capped single-line preview at the Full level, otherwise
/// type, source and time. A masked clip never shows its content.
String trayClipLabel(
  TrayClip clip,
  TrayPreviewLevel level,
  AppLocalizations l10n,
  String Function(DateTime) time,
) {
  final captured = time(clip.capturedAt);
  if (clip.masked) {
    return _joined([l10n.trayRecentSecret, clip.sourceApp, captured]);
  }
  final preview = _singleLine(clip.preview);
  if (level == TrayPreviewLevel.full && preview.isNotEmpty) {
    final characters = Characters(preview);
    return characters.length <= trayPreviewMaxCharacters
        ? preview
        : '${characters.take(trayPreviewMaxCharacters - 1).toString().trimRight()}…';
  }
  final kind = switch (clip.kind) {
    TrayClipKind.text => l10n.historyKindText,
    TrayClipKind.image => l10n.historyKindImage,
    TrayClipKind.file => l10n.historyKindFile,
    TrayClipKind.other => l10n.historyKindOther,
  };
  return _joined([kind, clip.sourceApp, captured]);
}

final _whitespace = RegExp(r'\s+');

String _singleLine(String? text) =>
    text == null ? '' : text.replaceAll(_whitespace, ' ').trim();

String _joined(List<String?> parts) => parts
    .whereType<String>()
    .map((part) => part.trim())
    .where((part) => part.isNotEmpty)
    .join(' · ');

String Function(DateTime) _timeFormat(String localeName) {
  DateFormat format;
  try {
    format = DateFormat.jm(localeName);
  } catch (_) {
    // Locale date symbols load with the application's Material delegate.
    format = DateFormat.jm('en_US');
  }
  return (value) => format.format(value.toLocal());
}
