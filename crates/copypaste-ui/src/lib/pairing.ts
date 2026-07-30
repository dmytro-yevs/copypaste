/**
 * The pairing payload — what a QR carries, and how it is read back.
 *
 * # Why this exists
 *
 * Pairing today is: one device mints a 52-character Crockford base32 code, and
 * the other device's user types it, plus a `host:port`, by hand. On a phone
 * with an on-screen keyboard that is not a feature anyone will use, and it is
 * the only way to pair.
 *
 * A camera carries the code better than fingers do. **One scan is the whole
 * pairing** — the code and the address travel together, because a QR that
 * carried only the code would leave the address to be typed and would have
 * solved the smaller half of the problem.
 *
 * # The payload is a credential
 *
 * `code` is the Noise pre-shared key in transferable form: anyone holding it
 * can pair with the minting device. So:
 *
 * * it is never logged and never toasted;
 * * it is never rendered as text — only as the QR's pixels (INV-13);
 * * it lives in component state for as long as the dialog does, and no longer.
 *
 * # Why not v1's QR format
 *
 * v1's `CPPAIR2.*` payload served a PAKE session with a TTL, and carried a
 * certificate fingerprint and a Supabase anon key alongside the password. v2
 * has none of those: pairing is Noise `NNpsk0`, where possession of the token
 * *is* the authentication (ADR-0002), and there is one transport. The format
 * below carries exactly what `pair_accept` takes and nothing else.
 *
 * That the v1 *flow* does not port is not an argument against a QR. The code
 * still has to cross the gap.
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

/**
 * Render a payload for a QR.
 *
 * A URI rather than JSON: it is compact (a shorter QR is a coarser QR, which
 * scans from further away and in worse light), and a phone's camera app that
 * recognises the scheme can hand it to the app directly.
 */
export function encodePairing(payload: PairingPayload): string {
  const params = new URLSearchParams({
    v: VERSION,
    c: payload.code,
    a: payload.addr,
  });
  return `${SCHEME}://${KIND}?${params.toString()}`;
}

/**
 * Read a scanned string back, or `null` if it is not one of ours.
 *
 * Everything is checked before anything is returned. A scanner sees whatever
 * happens to be in frame — a Wi-Fi QR, a payment code, a URL on a poster — and
 * a decoder that returned a half-parsed result would hand `pair_accept` a
 * "code" that came off a cereal box.
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
