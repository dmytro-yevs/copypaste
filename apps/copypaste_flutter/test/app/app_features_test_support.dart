import 'package:copypaste_flutter/app/app_features.dart';
import 'package:copypaste_flutter/features/ai_tools/controller/ai_tools_controller.dart';
import 'package:copypaste_flutter/features/content_rules/controller/content_rules_controller.dart';
import 'package:copypaste_flutter/features/paste_queue/controller/paste_queue_controller.dart';
import 'package:copypaste_flutter/features/settings/controller/settings_navigation_state.dart';
import 'package:copypaste_flutter/features/snippets/controller/snippets_controller.dart';
import 'package:copypaste_flutter/features/transforms/controller/transforms_settings_controller.dart';

import '../features/ai_tools/ai_tools_test_support.dart';
import '../features/content_rules/content_rule_test_support.dart';
import '../features/paste_queue/paste_queue_test_support.dart';
import '../features/snippets/snippet_test_support.dart';
import '../features/transforms/transform_test_support.dart';

/// [AppFeatures] over in-memory repositories, without platform services.
AppFeatures testAppFeatures({
  FakePasteQueueRepository? queue,
  void Function(SettingsSectionId section)? openSettings,
}) => AppFeatures(
  pasteQueue: PasteQueueController(
    repository: queue ?? FakePasteQueueRepository(),
    pasteHost: FakeForegroundPasteHost(),
    settle: Duration.zero,
  ),
  snippets: SnippetsController(repository: FakeSnippetRepository()),
  transforms: TransformsSettingsController(
    repository: FakeTransformRepository(),
    platform: 'macos',
  ),
  contentRules: ContentRulesController(repository: FakeContentRuleRepository()),
  aiTools: AiToolsController(
    repository: FakeAiToolsRepository(),
    serving: true,
  ),
  openSettings: openSettings ?? (_) {},
);
