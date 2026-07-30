import CopyPasteKit
import SwiftUI

/// Mint a pairing on this Mac and read the code out to the other device.
///
/// **The code is a secret.** It is the Noise pre-shared key in transferable
/// form: whoever has it can pair with this Mac and receive everything that is
/// synced. The handling here is deliberate, and each rule has a reason:
///
/// * **Hidden until asked for.** A pairing sheet that opens with a live
///   credential on screen is a credential in every screenshare and every
///   passer-by's line of sight. v1 blurred its QR for the same reason
///   (manifest 06 §3.3.1) and re-blurred on regeneration.
/// * **No "Copy code" button.** This is a clipboard manager: copying the
///   pairing secret would file it into the user's own history, where it would
///   then be synced to every paired device and shown in this very list. The
///   code is meant to be read out or typed.
/// * **Never logged**, never persisted, and dropped from memory when the sheet
///   closes.
/// * The screen it lives on is excluded from screen capture (`UtilityWindow`,
///   manifest 06 INV-35).
///
/// It is exposed to VoiceOver while revealed, in groups of four, because a
/// blind user has to be able to read it to the other device — an accessibility
/// contract and a security one pointing in opposite directions, resolved in
/// favour of the person who cannot see the screen. Revealing is an explicit
/// action either way.
struct ShowPairingCodeSheet: View {
    @Bindable var store: DevicesStore
    @Environment(\.dismiss) private var dismiss

    @State private var name = ""
    @State private var isRevealed = false

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("Pair a device").font(.title3.weight(.semibold))

            if let pairing = store.mintedPairing {
                minted(pairing)
            } else {
                request
            }

            if let error = store.actionError {
                MessageBox(title: error.message, detail: error.detail, isError: true)
            }

            HStack {
                Spacer()
                Button(store.mintedPairing == nil ? "Cancel" : "Done") { close() }
                    .keyboardShortcut(.cancelAction)
            }
        }
        .padding(18)
        .frame(width: 420)
        .onDisappear { store.discardMintedPairing() }
    }

    // MARK: - Before minting

    private var request: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("CopyPaste will create a one-time code. Type it into CopyPaste on the other device, along with this Mac’s address.")
                .font(.callout)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            TextField("Name for the other device", text: $name)
                .textFieldStyle(.roundedBorder)

            Button {
                Task { await store.createPairing(name: name) }
            } label: {
                if store.isPairing {
                    ProgressView().controlSize(.small)
                } else {
                    Text("Create Code")
                }
            }
            .buttonStyle(.borderedProminent)
            .disabled(store.isPairing)
        }
    }

    // MARK: - After minting

    private func minted(_ pairing: PairingSecret) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("On the other device, choose “Enter a Code” and type this in.")
                .font(.callout)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            GroupBox {
                VStack(alignment: .leading, spacing: 8) {
                    if isRevealed {
                        Text(pairing.groupedForDisplay.joined(separator: " "))
                            .font(.system(.title3, design: .monospaced))
                            .textSelection(.disabled)
                            .privacySensitive()
                            .accessibilityLabel(
                                "Pairing code: " + pairing.groupedForDisplay.joined(separator: ", ")
                            )
                        Text("Read this out or type it — don’t paste it anywhere.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    } else {
                        Button("Reveal code") { isRevealed = true }
                            .accessibilityHint("Shows a one-time pairing secret on screen")
                        Text("Hidden until you ask, so it isn’t on screen behind your back.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }

            if let address = pairing.listenAddress {
                LabeledContent("This Mac’s address") {
                    Text(address).font(.system(.body, design: .monospaced))
                }
            } else {
                Text("CopyPaste couldn’t work out this Mac’s address on the network. On the other device, use this Mac’s IP address and the CopyPaste port.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }

            LabeledContent("Pairing ID") {
                Text(pairing.pairingID.prefix(12))
                    .font(.system(.caption, design: .monospaced))
                    .foregroundStyle(.secondary)
            }
        }
    }

    private func close() {
        store.discardMintedPairing()
        store.dismissErrors()
        dismiss()
    }
}

/// Consume a code minted on another device.
struct EnterPairingCodeSheet: View {
    @Bindable var store: DevicesStore
    @Environment(\.dismiss) private var dismiss

    @State private var code = ""
    @State private var address = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("Enter a pairing code").font(.title3.weight(.semibold))
            Text("Type the code the other device is showing, and the address it gave you.")
                .font(.callout)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            // A secure field: the code is a credential, and this window may be
            // in a screenshare. It is typed once and never shown again.
            SecureField("Pairing code", text: $code)
                .textFieldStyle(.roundedBorder)
            TextField("Address (host:port)", text: $address)
                .textFieldStyle(.roundedBorder)
                .font(.system(.body, design: .monospaced))

            if let input = store.inputError {
                MessageBox(title: input, isError: true)
            } else if let error = store.actionError {
                MessageBox(title: error.message, detail: error.detail, isError: true)
            }

            HStack {
                Spacer()
                Button("Cancel") {
                    store.dismissErrors()
                    dismiss()
                }
                .keyboardShortcut(.cancelAction)

                Button {
                    Task {
                        if await store.acceptPairing(code: code, address: address) {
                            // The code has been used; do not leave it in a
                            // field for the next person to read.
                            code = ""
                            dismiss()
                        }
                    }
                } label: {
                    if store.isPairing {
                        ProgressView().controlSize(.small)
                    } else {
                        Text("Pair")
                    }
                }
                .buttonStyle(.borderedProminent)
                .keyboardShortcut(.defaultAction)
                .disabled(store.isPairing)
            }
        }
        .padding(18)
        .frame(width: 420)
        .onDisappear { code = "" }
    }
}
