import 'package:copypaste_flutter/features/devices/devices.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  List<PairingStepStatus> statuses(PairingProgress progress) => [
    for (var index = 0; index < progress.steps.length; index++)
      progress.statusOf(index),
  ];

  const upcoming = PairingStepStatus.upcoming;
  const current = PairingStepStatus.current;
  const completed = PairingStepStatus.completed;
  const failed = PairingStepStatus.failed;

  test('names the first step after the entry flow', () {
    const shared = [
      PairingStep.connect,
      PairingStep.confirm,
      PairingStep.paired,
    ];
    for (final (mode, entry) in [
      (PairingEntryMode.invite, PairingStep.invite),
      (PairingEntryMode.scanQr, PairingStep.scan),
      (PairingEntryMode.enterCode, PairingStep.code),
      (PairingEntryMode.nearby, PairingStep.request),
    ]) {
      expect(PairingProgress.resolve(mode: mode, starting: true).steps, [
        entry,
        ...shared,
      ]);
    }
  });

  test('keeps the entry step current until the runtime has a ceremony', () {
    for (final progress in [
      PairingProgress.resolve(mode: PairingEntryMode.invite, starting: true),
      PairingProgress.resolve(mode: PairingEntryMode.nearby, starting: true),
      PairingProgress.resolve(mode: PairingEntryMode.enterCode),
      PairingProgress.resolve(mode: PairingEntryMode.scanQr),
      // A rejected code or QR leaves the form and the scanner usable.
      PairingProgress.resolve(mode: PairingEntryMode.enterCode, hasError: true),
      PairingProgress.resolve(mode: PairingEntryMode.scanQr, hasError: true),
      PairingProgress.resolve(
        mode: PairingEntryMode.invite,
        state: PairingState.waitingForPeer,
      ),
    ]) {
      expect(statuses(progress), [current, upcoming, upcoming, upcoming]);
      expect(progress.outcome, isNull);
      expect(progress.localDecision, isNull);
      expect(progress.peerDecision, isNull);
    }
  });

  test('a submitted code or decoded QR moves a joining device to Connect', () {
    for (final progress in [
      PairingProgress.resolve(mode: PairingEntryMode.enterCode, starting: true),
      PairingProgress.resolve(mode: PairingEntryMode.scanQr, starting: true),
      // The system scanner opens the pane only after it has read a code.
      PairingProgress.resolve(
        mode: PairingEntryMode.scanQr,
        systemScanner: true,
      ),
      // A joiner holds a ceremony only once its entry step succeeded.
      PairingProgress.resolve(
        mode: PairingEntryMode.enterCode,
        state: PairingState.waitingForPeer,
      ),
      PairingProgress.resolve(
        mode: PairingEntryMode.invite,
        state: PairingState.handshaking,
      ),
    ]) {
      expect(statuses(progress), [completed, current, upcoming, upcoming]);
      expect(progress.outcome, isNull);
    }
  });

  test('a start that cannot be retried in the pane fails its step', () {
    for (final mode in [PairingEntryMode.invite, PairingEntryMode.nearby]) {
      final progress = PairingProgress.resolve(mode: mode, hasError: true);
      expect(statuses(progress), [failed, upcoming, upcoming, upcoming]);
      expect(progress.outcome, PairingState.failed);
    }
    final scanned = PairingProgress.resolve(
      mode: PairingEntryMode.scanQr,
      systemScanner: true,
      hasError: true,
    );
    expect(statuses(scanned), [completed, failed, upcoming, upcoming]);
    expect(scanned.outcome, PairingState.failed);
  });

  test('confirmation and success advance every flow alike', () {
    for (final mode in PairingEntryMode.values) {
      final confirming = PairingProgress.resolve(
        mode: mode,
        state: PairingState.awaitingConfirmation,
      );
      expect(statuses(confirming), [completed, completed, current, upcoming]);
      expect(confirming.activeIndex, 2);

      final paired = PairingProgress.resolve(
        mode: mode,
        state: PairingState.confirmed,
      );
      expect(statuses(paired), everyElement(completed));
      expect(paired.activeIndex, 4);
      expect(paired.outcome, PairingState.confirmed);
      expect(paired.failed, isFalse);
    }
  });

  test('a rejection always ends the confirmation step', () {
    // The runtime rejects only inside the decision exchange, so the step is
    // certain even when polling never observed the confirmation state.
    final progress = PairingProgress.resolve(
      mode: PairingEntryMode.enterCode,
      state: PairingState.rejected,
      furthestState: PairingState.handshaking,
    );
    expect(statuses(progress), [completed, completed, failed, upcoming]);
    expect(progress.outcome, PairingState.rejected);
    expect(progress.failed, isTrue);
  });

  test('other failures end the furthest step that was observed', () {
    for (final outcome in [
      PairingState.cancelled,
      PairingState.timedOut,
      PairingState.failed,
    ]) {
      PairingProgress at(PairingEntryMode mode, PairingState? furthest) =>
          PairingProgress.resolve(
            mode: mode,
            state: outcome,
            furthestState: furthest,
          );

      final waiting = at(PairingEntryMode.invite, PairingState.waitingForPeer);
      expect(statuses(waiting), [failed, upcoming, upcoming, upcoming]);
      expect(waiting.outcome, outcome);
      expect(waiting.localDecision, isNull);

      // No later state was seen, yet a joiner's entry step did succeed.
      final joining = at(PairingEntryMode.scanQr, null);
      expect(statuses(joining), [completed, failed, upcoming, upcoming]);

      final connecting = at(PairingEntryMode.nearby, PairingState.handshaking);
      expect(statuses(connecting), [completed, failed, upcoming, upcoming]);
      expect(connecting.peerDecision, isNull);

      final confirming = at(
        PairingEntryMode.invite,
        PairingState.awaitingConfirmation,
      );
      expect(statuses(confirming), [completed, completed, failed, upcoming]);
      expect(confirming.localDecision, PairingLocalDecision.undecided);
      expect(confirming.peerDecision, PairingPeerDecision.unknown);
    }
  });

  test('tracks the furthest non-terminal state', () {
    var furthest = PairingProgress.furthest(null, PairingState.waitingForPeer);
    expect(furthest, PairingState.waitingForPeer);
    furthest = PairingProgress.furthest(
      furthest,
      PairingState.awaitingConfirmation,
    );
    furthest = PairingProgress.furthest(furthest, PairingState.handshaking);
    expect(furthest, PairingState.awaitingConfirmation);
    expect(
      PairingProgress.furthest(furthest, PairingState.failed),
      PairingState.awaitingConfirmation,
    );
    expect(PairingProgress.furthest(null, PairingState.timedOut), isNull);
  });

  group('decisions', () {
    PairingProgress confirming({
      PairingState state = PairingState.awaitingConfirmation,
      bool decisionInFlight = false,
      bool? decisionRequest,
    }) => PairingProgress.resolve(
      mode: PairingEntryMode.enterCode,
      state: state,
      furthestState: PairingState.awaitingConfirmation,
      decisionInFlight: decisionInFlight,
      decisionRequest: decisionRequest,
    );

    test('the other answer stays unknown until this device confirmed', () {
      final undecided = confirming();
      expect(undecided.localDecision, PairingLocalDecision.undecided);
      expect(undecided.peerDecision, PairingPeerDecision.unknown);

      final sending = confirming(decisionInFlight: true, decisionRequest: true);
      expect(sending.localDecision, PairingLocalDecision.sending);
      expect(sending.peerDecision, PairingPeerDecision.unknown);

      // Still awaiting after a delivered confirmation: the peer has not
      // answered, or the ceremony would have ended.
      final confirmed = confirming(decisionRequest: true);
      expect(confirmed.localDecision, PairingLocalDecision.confirmed);
      expect(confirmed.peerDecision, PairingPeerDecision.waiting);
    });

    test('a failed rejection attempt leaves this device undecided', () {
      final retry = confirming(decisionRequest: false);
      expect(retry.localDecision, PairingLocalDecision.undecided);
      expect(retry.peerDecision, PairingPeerDecision.unknown);
    });

    test('success means both parties confirmed', () {
      final paired = confirming(state: PairingState.confirmed);
      expect(paired.localDecision, PairingLocalDecision.confirmed);
      expect(paired.peerDecision, PairingPeerDecision.confirmed);
    });

    test('a rejection is attributed only when this device sent it', () {
      // The other device rejecting and closing its pane look the same.
      for (final request in [null, true]) {
        final elsewhere = confirming(
          state: PairingState.rejected,
          decisionRequest: request,
        );
        expect(
          elsewhere.localDecision,
          request == true
              ? PairingLocalDecision.confirmed
              : PairingLocalDecision.undecided,
        );
        expect(elsewhere.peerDecision, PairingPeerDecision.unknown);
        expect(elsewhere.endedUnattributed, isTrue);
      }

      final byThisDevice = confirming(
        state: PairingState.rejected,
        decisionRequest: false,
      );
      expect(byThisDevice.localDecision, PairingLocalDecision.rejected);
      expect(byThisDevice.peerDecision, PairingPeerDecision.unknown);
      expect(byThisDevice.endedUnattributed, isFalse);
    });

    test('a timeout after confirming does not invent the other answer', () {
      final timedOut = confirming(
        state: PairingState.timedOut,
        decisionRequest: true,
      );
      expect(timedOut.localDecision, PairingLocalDecision.confirmed);
      expect(timedOut.peerDecision, PairingPeerDecision.unknown);
    });
  });
}
