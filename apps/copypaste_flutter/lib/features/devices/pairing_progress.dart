import 'devices_gateway.dart';

/// A stage of the pairing ceremony, named after what the runtime does in it.
enum PairingStep { invite, scan, code, request, connect, confirm, paired }

enum PairingStepStatus { upcoming, current, completed, failed }

/// The decision made on this device during bilateral confirmation.
enum PairingLocalDecision { undecided, sending, confirmed, rejected }

/// The other party's decision as far as this device can tell.
///
/// The runtime reports the remote decision only through the terminal state and
/// never says who ended a ceremony, so [unknown] covers every other moment.
enum PairingPeerDecision { unknown, waiting, confirmed }

/// Where a pairing ceremony stands, derived only from observed runtime state.
class PairingProgress {
  const PairingProgress._({
    required this.steps,
    required this.activeIndex,
    required this.outcome,
    required this.localDecision,
    required this.peerDecision,
  });

  /// [state] is null until the runtime returns a ceremony. [furthestState] is
  /// the latest non-terminal state seen, which places an ambiguous failure.
  /// [decisionRequest] is the answer asked for on this device, if any.
  factory PairingProgress.resolve({
    required PairingEntryMode mode,
    PairingState? state,
    PairingState? furthestState,
    bool starting = false,
    bool hasError = false,
    bool systemScanner = false,
    bool decisionInFlight = false,
    bool? decisionRequest,
  }) {
    final (index, outcome) = state == null
        ? _beforeCeremony(mode, starting, hasError, systemScanner)
        : _duringCeremony(mode, state, furthestState);
    final decided =
        state != null &&
        (index == _confirmIndex || state == PairingState.confirmed);
    return PairingProgress._(
      steps: [_entryStep(mode), ..._sharedSteps],
      activeIndex: index,
      outcome: outcome,
      localDecision: decided
          ? _localDecision(state, decisionInFlight, decisionRequest)
          : null,
      peerDecision: decided
          ? _peerDecision(state, decisionInFlight, decisionRequest)
          : null,
    );
  }

  static const _sharedSteps = [
    PairingStep.connect,
    PairingStep.confirm,
    PairingStep.paired,
  ];
  static const _entryIndex = 0;
  static const _connectIndex = 1;
  static const _confirmIndex = 2;
  static const _stepCount = 4;

  final List<PairingStep> steps;

  /// The current step, the step that failed, or [steps] length once paired.
  final int activeIndex;

  /// The terminal state, or null while the ceremony can still advance.
  final PairingState? outcome;

  bool get failed => outcome != null && outcome != PairingState.confirmed;

  /// A rejection this device did not send. The runtime does not say whether
  /// the other device rejected or closed its pane, so it is not attributed.
  bool get endedUnattributed =>
      outcome == PairingState.rejected &&
      localDecision != PairingLocalDecision.rejected;

  /// Null until the ceremony reaches bilateral confirmation.
  final PairingLocalDecision? localDecision;
  final PairingPeerDecision? peerDecision;

  PairingStepStatus statusOf(int index) {
    if (index < activeIndex) return PairingStepStatus.completed;
    if (index > activeIndex) return PairingStepStatus.upcoming;
    return failed ? PairingStepStatus.failed : PairingStepStatus.current;
  }

  /// The later of two observed states; terminal states never count as reached.
  static PairingState? furthest(PairingState? previous, PairingState next) {
    if (next.isTerminal) return previous;
    return previous == null || next.index > previous.index ? next : previous;
  }

  static PairingStep _entryStep(PairingEntryMode mode) => switch (mode) {
    PairingEntryMode.invite => PairingStep.invite,
    PairingEntryMode.scanQr => PairingStep.scan,
    PairingEntryMode.enterCode => PairingStep.code,
    PairingEntryMode.nearby => PairingStep.request,
  };

  static (int, PairingState?) _beforeCeremony(
    PairingEntryMode mode,
    bool starting,
    bool hasError,
    bool systemScanner,
  ) => switch (mode) {
    // These panes offer no retry, so a start that is not running has failed.
    PairingEntryMode.invite || PairingEntryMode.nearby => (
      _entryIndex,
      starting ? null : PairingState.failed,
    ),
    PairingEntryMode.enterCode => (
      starting ? _connectIndex : _entryIndex,
      null,
    ),
    // The system scanner hands over only after it has read a code.
    PairingEntryMode.scanQr when systemScanner => (
      _connectIndex,
      !starting && hasError ? PairingState.failed : null,
    ),
    PairingEntryMode.scanQr => (starting ? _connectIndex : _entryIndex, null),
  };

  static (int, PairingState?) _duringCeremony(
    PairingEntryMode mode,
    PairingState state,
    PairingState? furthestState,
  ) {
    // A joining device holds a ceremony only after its entry step succeeded.
    final floor = mode == PairingEntryMode.invite ? _entryIndex : _connectIndex;
    int reached(PairingState? observed) {
      final index = switch (observed) {
        PairingState.awaitingConfirmation => _confirmIndex,
        PairingState.handshaking => _connectIndex,
        _ => _entryIndex,
      };
      return index < floor ? floor : index;
    }

    return switch (state) {
      PairingState.confirmed => (_stepCount, state),
      // Only the decision exchange can end in a rejection.
      PairingState.rejected => (_confirmIndex, state),
      PairingState.cancelled ||
      PairingState.timedOut ||
      PairingState.failed => (reached(furthestState), state),
      _ => (reached(state), null),
    };
  }

  static PairingLocalDecision _localDecision(
    PairingState state,
    bool decisionInFlight,
    bool? decisionRequest,
  ) {
    if (state == PairingState.confirmed) return PairingLocalDecision.confirmed;
    if (state == PairingState.rejected && decisionRequest == false) {
      return PairingLocalDecision.rejected;
    }
    if (decisionInFlight) return PairingLocalDecision.sending;
    return decisionRequest == true
        ? PairingLocalDecision.confirmed
        : PairingLocalDecision.undecided;
  }

  static PairingPeerDecision _peerDecision(
    PairingState state,
    bool decisionInFlight,
    bool? decisionRequest,
  ) => switch (state) {
    PairingState.confirmed => PairingPeerDecision.confirmed,
    PairingState.awaitingConfirmation
        when decisionRequest == true && !decisionInFlight =>
      PairingPeerDecision.waiting,
    _ => PairingPeerDecision.unknown,
  };
}
