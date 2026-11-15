import 'package:copypaste_flutter/app/theme/app_icon_frame.dart';
import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../app/theme/app_theme.dart';
import '../../app/theme/app_tokens.dart';
import 'device_presentation.dart';
import 'pairing_progress.dart';

/// The confirmation decision of each party, as far as this device knows it.
class PairingDecisions extends StatelessWidget {
  const PairingDecisions({
    super.key,
    required this.local,
    required this.peer,
    this.peerName,
  });

  final PairingLocalDecision local;
  final PairingPeerDecision peer;
  final String? peerName;

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return Semantics(
      container: true,
      liveRegion: true,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          _row(
            context,
            key: const ValueKey<String>('pairing-decision-local'),
            party: l10n.devicesThisDevice,
            icon: DevicePresentation.pairingLocalDecisionIcon(local),
            tone: DevicePresentation.pairingLocalDecisionTone(local),
            status: DevicePresentation.pairingLocalDecisionLabel(local),
          ),
          const Divider(),
          _row(
            context,
            key: const ValueKey<String>('pairing-decision-peer'),
            party: peerName ?? l10n.pairingOtherDevice,
            icon: DevicePresentation.pairingPeerDecisionIcon(peer),
            tone: DevicePresentation.pairingPeerDecisionTone(peer),
            status: DevicePresentation.pairingPeerDecisionLabel(peer),
          ),
        ],
      ),
    );
  }

  Widget _row(
    BuildContext context, {
    required Key key,
    required String party,
    required IconData icon,
    required AppStatusTone tone,
    required String status,
  }) => MergeSemantics(
    key: key,
    child: Basic(
      theme: AppTheme.deviceListRowTheme,
      leading: AppIconFrame(
        size: AppIconSize.md,
        child: Icon(
          icon,
          size: AppIconSize.md,
          color: AppTheme.statusColor(context, tone),
        ),
      ),
      title: Text(party, maxLines: 1, overflow: TextOverflow.ellipsis),
      content: Text(status),
    ),
  );
}
