/**
 * **The payload is a credential.** `code` is the Noise pre-shared key in
 * transferable form — possession of it *is* the authentication (ADR-0002) — so
 * it is never logged, never toasted, never rendered as text but only as the
 * QR's pixels (INV-13), and lives in component state no longer than the dialog.
 *
 * The code and the address travel together, so one scan is the whole pairing.
 */

/** Identifies our payload, and its version. A scanner that meets some other
 *  application's QR must reject it rather than feed it to `pair_accept`. */
const SCHEME = "copypaste";
const KIND = "pair";
const VERSION = "2";

export interface PairingPayload {
  readonly code: string;
  /** `host:port` for the accepting device to dial. */
  readonly addr: string;
}

/** A URI rather than JSON: it is compact, and a shorter QR is a coarser QR,
 *  which scans from further away and in worse light. */
export function encodePairing(payload: PairingPayload): string {
  const params = new URLSearchParams({
    v: VERSION,
    c: payload.code,
    a: payload.addr,
  });
  return `${SCHEME}://${KIND}?${params.toString()}`;
}

/**
 * Read a scanned string back, or `null` if it is not one of ours. Everything is
 * checked before anything is returned: a camera sees whatever is in frame, and
 * a half-parsed result would hand `pair_accept` a "code" off a cereal box.
 */
export function decodePairing(scanned: string): PairingPayload | null {
  let url: URL;
  try {
    url = new URL(scanned.trim());
  } catch {
    return null;
  }

  // `URL` keeps the colon on the protocol.
  if (url.protocol !== `${SCHEME}:`) return null;
  if (url.hostname !== KIND) return null;
  if (url.searchParams.get("v") !== VERSION) return null;

  const code = url.searchParams.get("c");
  const addr = url.searchParams.get("a");
  if (!code || !addr) return null;
  // A host and a port. Not a full parse — the daemon does that, and it is the
  // side that has to be strict — but enough that an empty or obviously wrong
  // value does not reach it.
  if (!/^[^\s/]+:\d{1,5}$/.test(addr)) return null;

  return { code, addr };
}
