import 'package:copypaste_flutter/generated/api.dart' as runtime;
import 'package:copypaste_flutter/generated/api/clips.dart' as clips;
import 'package:copypaste_flutter/generated/api/rules.dart' as rules;
import 'package:flutter/services.dart';

import '../../../shared/clip_visibility.dart';
import '../models/tray_menu_model.dart';
import 'tray_clip_source.dart';

class RuntimeTrayClipSource implements TrayClipSource {
  const RuntimeTrayClipSource();

  @override
  Future<List<TrayClip>> recent(int limit) async {
    final page = await runtime.listClips(limit: limit);
    final now = DateTime.now();
    return [for (final clip in page.clips) ?trayClipOf(clip, now)];
  }

  @override
  Future<TrayClip?> latestFromOtherDevice() async {
    final clip = await clips.latestClipFromOtherDevice(copy: false);
    return clip == null ? null : trayClipOf(clip, DateTime.now());
  }

  @override
  Future<void> copy(String id) => runtime.copyClip(id: id);

  @override
  Future<bool> captureClipboard() async {
    final text = (await Clipboard.getData(Clipboard.kTextPlain))?.text;
    if (text == null || text.trim().isEmpty) return false;
    await clips.createManualClip(text: text);
    return true;
  }

  @override
  Future<List<TraySkipEntry>> recentlySkipped() async {
    final events = await rules.contentRuleSkipLog();
    return [
      for (final event in events)
        TraySkipEntry(
          at: DateTime.fromMillisecondsSinceEpoch(event.atMs),
          ruleName: event.ruleName,
          sourceApp: event.sourceAppName,
        ),
    ]..sort((left, right) => right.at.compareTo(left.at));
  }
}

/// The tray view of [clip], or null when it expired and awaits its purge.
TrayClip? trayClipOf(runtime.Clip clip, DateTime now) {
  final annotations = clip.annotations;
  final expiresAt = annotations?.expiresAtMs;
  final visibility = ClipVisibility(
    secret: clip.secret,
    ruleSecret: annotations?.ruleSecret ?? false,
    expiresAt: expiresAt == null
        ? null
        : DateTime.fromMillisecondsSinceEpoch(expiresAt),
  );
  final presentation = visibility.presentationAt(now);
  if (presentation == ClipPresentation.hidden) return null;
  final masked = presentation == ClipPresentation.masked;
  final kind = switch (clip.contentClass) {
    runtime.ClipContentClass.text => TrayClipKind.text,
    runtime.ClipContentClass.image => TrayClipKind.image,
    runtime.ClipContentClass.file => TrayClipKind.file,
    runtime.ClipContentClass.other => TrayClipKind.other,
  };
  return TrayClip(
    id: clip.id,
    kind: kind,
    masked: masked,
    capturedAt: DateTime.fromMillisecondsSinceEpoch(clip.createdAtMs),
    preview: masked || kind != TrayClipKind.text ? null : clip.content,
    sourceApp: clip.sourceAppName,
  );
}
