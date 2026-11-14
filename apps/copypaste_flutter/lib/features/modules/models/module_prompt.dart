/// A question a module is waiting for the user to answer.
class ModulePrompt {
  const ModulePrompt({
    required this.id,
    required this.moduleId,
    required this.title,
    required this.body,
    required this.choices,
    required this.expiresAt,
  });
  final String id;
  final String moduleId;
  final String title;
  final String body;
  final List<ModulePromptChoice> choices;
  final DateTime expiresAt;
}

class ModulePromptChoice {
  const ModulePromptChoice({
    required this.id,
    required this.title,
    this.destructive = false,
  });
  final String id;
  final String title;
  final bool destructive;
}
