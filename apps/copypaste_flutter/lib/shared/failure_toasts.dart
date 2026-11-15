import 'package:flutter/scheduler.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../app/theme/app_toast.dart';
import '../l10n/l10n.dart';

/// Shows each new action failure that [listenable] reports as an error toast.
///
/// An action failure never takes space in the layout. [failure] reads the
/// current failure text (already user-facing, see `ErrorText`); a toast
/// appears when it changes to a new non-null value. Return null from
/// [failure] while the failure belongs to a load state that a `StateView`
/// already shows in place of the content.
class FailureToasts extends StatefulWidget {
  const FailureToasts({
    super.key,
    required this.listenable,
    required this.failure,
    this.title,
    required this.child,
  });

  final Listenable listenable;
  final String? Function() failure;

  /// What failed; defaults to the generic "Action failed".
  final String Function(BuildContext context)? title;
  final Widget child;

  @override
  State<FailureToasts> createState() => _FailureToastsState();
}

class _FailureToastsState extends State<FailureToasts> {
  String? _shown;

  @override
  void initState() {
    super.initState();
    _shown = widget.failure();
    widget.listenable.addListener(_changed);
  }

  @override
  void didUpdateWidget(FailureToasts oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (identical(oldWidget.listenable, widget.listenable)) return;
    oldWidget.listenable.removeListener(_changed);
    widget.listenable.addListener(_changed);
    _shown = widget.failure();
  }

  @override
  void dispose() {
    widget.listenable.removeListener(_changed);
    super.dispose();
  }

  void _changed() {
    final failure = widget.failure();
    if (failure == _shown) return;
    _shown = failure;
    if (failure == null || !mounted) return;
    if (SchedulerBinding.instance.schedulerPhase ==
        SchedulerPhase.persistentCallbacks) {
      SchedulerBinding.instance.addPostFrameCallback((_) => _show(failure));
    } else {
      _show(failure);
    }
  }

  void _show(String failure) {
    if (!mounted) return;
    AppToast.show(
      context,
      title: widget.title?.call(context) ?? context.l10n.appActionFailed,
      message: failure,
      tone: AppToastTone.error,
    );
  }

  @override
  Widget build(BuildContext context) => widget.child;
}
