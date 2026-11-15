import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../models/module_view.dart';

extension ModuleIconData on ModuleIcon {
  IconData get data => switch (this) {
    ModuleIcon.notebookText => LucideIcons.notebookText,
    ModuleIcon.filePlus => LucideIcons.filePlus,
    ModuleIcon.bookmark => LucideIcons.bookmark,
    ModuleIcon.sparkles => LucideIcons.sparkles,
    ModuleIcon.puzzle => LucideIcons.puzzle,
    ModuleIcon.zap => LucideIcons.zap,
  };
}
