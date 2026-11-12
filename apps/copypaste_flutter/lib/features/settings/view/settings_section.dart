import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_motion.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';

/// A titled settings section; grouped content shares one card.
class SettingsSection extends StatelessWidget {
  const SettingsSection({
    super.key,
    required this.title,
    required this.description,
    required this.children,
    this.groupContent = true,
    this.showHeading = true,
  });

  final String title;
  final String description;
  final List<Widget> children;
  final bool groupContent;
  final bool showHeading;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (showHeading) ...[
          Text(title, style: Theme.of(context).typography.h3),
          const Gap(AppSpacing.xs),
        ],
        Text(description).muted(),
        const Gap(AppSpacing.lg),
        if (groupContent) SettingsGroup(children: children) else ...children,
      ],
    );
  }
}

/// One card of divided settings rows.
class SettingsGroup extends StatelessWidget {
  const SettingsGroup({super.key, required this.children});

  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    return Card(
      theme: AppTheme.settingsGroupCardTheme,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          for (var i = 0; i < children.length; i++) ...[
            if (i > 0) const Divider(),
            children[i],
          ],
        ],
      ),
    );
  }
}

/// A setting title and description with its control.
///
/// Controls other than a switch stack below the copy when space is narrow or
/// text is scaled up.
class SettingRow extends StatelessWidget {
  const SettingRow({
    super.key,
    required this.title,
    required this.description,
    required this.trailing,
    this.highlighted = false,
  });

  final String title;
  final String description;
  final Widget trailing;
  final bool highlighted;

  @override
  Widget build(BuildContext context) {
    final copy = Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(title).medium(),
        const Gap(AppSpacing.xs),
        Text(description).muted().textSmall(),
      ],
    );
    return OutlinedContainer(
      key: ValueKey<String>('settings-row-$title'),
      duration: AppMotion.resolve(context, AppMotion.quick),
      theme: AppTheme.settingsRowTheme(context, highlighted: highlighted),
      clipBehavior: Clip.none,
      child: LayoutBuilder(
        builder: (context, constraints) {
          final stack =
              trailing is! Switch &&
              (constraints.maxWidth <
                      AppLayoutSize.settingsStackedControlWidth ||
                  MediaQuery.textScalerOf(context).scale(1) > 1.3);
          if (stack) {
            return Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                copy,
                const Gap(AppSpacing.md),
                Align(alignment: Alignment.centerRight, child: trailing),
              ],
            );
          }
          return Row(
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              Expanded(child: copy),
              const Gap(AppSpacing.lg),
              trailing,
            ],
          );
        },
      ),
    );
  }
}
