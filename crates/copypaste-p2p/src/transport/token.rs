//! The pairing token: the 256-bit shared secret that makes two devices a pair,
//! and the human-transferable code it prints as.
//!
//! Pure — no sockets, no filesystem. The token is the *input* to the handshake
//! in [`super::handshake`]; nothing here knows Noise exists beyond borrowing
//! its BLAKE2s.

use std::fmt;
use std::sync::OnceLock;

use data_encoding::{Encoding, Specification};
use rand::rngs::OsRng;
use rand::RngCore;
use snow::params::HashChoice;
use snow::resolvers::{CryptoResolver, DefaultResolver};
use subtle::ConstantTimeEq;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use super::TransportError;

/// Length of the pairing token, and of the Noise pre-shared key it becomes.
pub const TOKEN_LEN: usize = 32;

/// Characters per group in the printed pairing code.
const CODE_GROUP: usize = 4;

/// Group separator in the printed pairing code. Ignored on parse, along with
/// whitespace and `_`, so a user may retype the code however they like.
const CODE_SEPARATOR: char = '-';

/// Domain separator for [`PairingToken::pairing_id`]. Distinct from every other
/// use of the token, so the public id and the secret PSK are not derived by the
/// same function (port manifest 02, I-16).
const PAIRING_ID_DOMAIN: &[u8] = b"copypaste/v2/pairing-id|";

/// Bytes of the pairing-id digest that are kept: 128 bits, collision-free for a
/// user's handful of devices and short enough to read in a log line.
const PAIRING_ID_LEN: usize = 16;

/// The shared secret that makes two devices a pair.
///
/// 256 bits from the OS CSPRNG. Possession of it *is* the authentication.
/// Zeroized on drop, compared in constant time, never rendered by `Debug`, and
/// intentionally not `Clone` (port manifest 02, I-12): copying key material
/// should be an explicit act, which is what [`PairingToken::psk`] is.
pub struct PairingToken(Zeroizing<[u8; TOKEN_LEN]>);

impl PairingToken {
    /// Mint a fresh token. `OsRng`, never `thread_rng` — one entropy source
    /// across the whole crypto surface (port manifest 02, I-11).
    #[must_use]
    pub fn generate() -> Self {
        let mut bytes = Zeroizing::new([0u8; TOKEN_LEN]);
        OsRng.fill_bytes(bytes.as_mut_slice());
        Self(bytes)
    }

    /// Build a token from bytes obtained elsewhere. The caller asserts they are
    /// 256 bits of CSPRNG output; everything downstream rests on that.
    #[must_use]
    pub fn from_bytes(bytes: &[u8; TOKEN_LEN]) -> Self {
        Self(Zeroizing::new(*bytes))
    }

    /// The human-transferable form: 52 Crockford base32 characters in groups of
    /// four, e.g. `0G7M-XQ4V-…`.
    ///
    /// Crockford rather than RFC 4648 because this string gets read aloud and
    /// retyped: no `I`, `L`, `O` or `U`, so no `0`/`O` or `1`/`I`/`l` confusion
    /// and no accidental profanity. [`PairingToken::parse`] folds case and
    /// translates the ambiguous glyphs anyway.
    ///
    /// The code is a secret: safe to *show*, never to log or put in an error.
    #[must_use]
    pub fn to_code(&self) -> String {
        let raw = code_encoding().encode(&self.0[..]);
        let groups = raw.len().div_ceil(CODE_GROUP);
        let mut out = String::with_capacity(raw.len() + groups);
        for (i, chunk) in raw.as_bytes().chunks(CODE_GROUP).enumerate() {
            if i > 0 {
                out.push(CODE_SEPARATOR);
            }
            // `chunk` came out of `encode`, which only emits ASCII symbols.
            out.push_str(std::str::from_utf8(chunk).unwrap_or_default());
        }
        out
    }

    /// Parse a pairing code produced by [`PairingToken::to_code`]. Tolerant of
    /// how a human retypes it — case folded, `-`/`_`/whitespace ignored,
    /// `O`/`o` → `0`, `I`/`i`/`L`/`l` → `1` — and of nothing else: a character
    /// outside the alphabet, the wrong decoded length or non-zero trailing bits
    /// is rejected rather than silently coerced.
    ///
    /// # Errors
    ///
    /// [`TransportError::InvalidCode`], with no detail about the input.
    pub fn parse(code: &str) -> Result<Self, TransportError> {
        let mut decoded = code_encoding()
            .decode(code.as_bytes())
            .map_err(|_| TransportError::InvalidCode)?;
        if decoded.len() != TOKEN_LEN {
            decoded.zeroize();
            return Err(TransportError::InvalidCode);
        }
        let mut bytes = Zeroizing::new([0u8; TOKEN_LEN]);
        bytes.copy_from_slice(&decoded);
        decoded.zeroize();
        Ok(Self(bytes))
    }

    /// The token as a Noise pre-shared key. A copy of the secret, *not*
    /// zeroized for you: wrap it in `zeroize::Zeroizing` if it outlives the
    /// call that needs it.
    #[must_use]
    pub fn psk(&self) -> [u8; TOKEN_LEN] {
        *self.0
    }

    /// A stable, non-secret identifier for this pairing: 32 lowercase hex
    /// characters, safe to log and to use as the [`crate::PeerStore`] key.
    ///
    /// `BLAKE2s(domain || token)`, truncated to 128 bits — BLAKE2s because the
    /// Noise suite already provides it, so no second hash implementation enters
    /// the tree (`AGENTS.md` rule 1). One-way on purpose: recovering the token
    /// would mean preimaging a 256-bit random input, so the id is safe to log.
    /// It identifies a pairing; it does not stand in for it.
    #[must_use]
    pub fn pairing_id(&self) -> String {
        let digest = blake2s(&[PAIRING_ID_DOMAIN, &self.0[..]]);
        hex::encode(&digest[..PAIRING_ID_LEN])
    }
}

/// One stored pairing offered to [`Session::accept_any`](super::Session::accept_any)
/// as a candidate for an inbound handshake.
///
/// # Why this is not a plain tuple
///
/// The responder has to copy every pairing key it holds before it knows who is
/// dialling, so an attacker who never completes a handshake can drive that copy
/// as fast as they can open TCP sockets. The copies are therefore wiped when
/// they go out of scope, and [`crate::PeerStore::psks`] hands the set out inside
/// `Zeroizing` so a caller cannot lose the wipe by binding it to a plain local —
/// which is exactly what the listener used to do (security review F-8).
///
/// Not `Clone`, for the same reason [`PairingToken`] is not: every copy of key
/// material should be a deliberate act.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct PskCandidate {
    /// The non-secret pairing id this key belongs to.
    pub pairing_id: String,
    /// The Noise pre-shared key. **This is the secret.**
    pub psk: [u8; TOKEN_LEN],
}

/// Prints the pairing id and nothing else, so the type can sit inside a
/// `#[derive(Debug)]` struct without the key following it into a log.
impl fmt::Debug for PskCandidate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PskCandidate")
            .field("pairing_id", &self.pairing_id)
            .finish_non_exhaustive()
    }
}

/// Constant-time. `==` on key bytes short-circuits at the first differing byte
/// and leaks the matching-prefix length by timing (port manifest 02, I-13);
/// routing `PartialEq` through `subtle` means no caller can reach a
/// short-circuiting comparison of token bytes.
impl PartialEq for PairingToken {
    fn eq(&self, other: &Self) -> bool {
        self.0[..].ct_eq(&other.0[..]).into()
    }
}

impl Eq for PairingToken {}

/// Prints the pairing id and nothing else. v1 had no `Debug` at all; a `Debug`
/// structurally incapable of reaching the secret gets the same property while
/// letting the type sit inside a `#[derive(Debug)]` struct, which is where an
/// accidental `{:?}` on a secret normally comes from.
impl fmt::Debug for PairingToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PairingToken")
            .field("pairing_id", &self.pairing_id())
            .finish_non_exhaustive()
    }
}

/// Crockford base32, built once: digits plus the 22 letters that are not `I`,
/// `L`, `O` or `U`; lowercase folded and the ambiguous glyphs translated;
/// `-`, `_` and whitespace ignored so grouping and line wrapping survive a chat
/// window; no padding and `check_trailing_bits` left `true`, so a given token
/// has exactly one valid encoding.
fn code_encoding() -> &'static Encoding {
    static ENCODING: OnceLock<Encoding> = OnceLock::new();
    ENCODING.get_or_init(|| {
        let mut spec = Specification::new();
        spec.symbols.push_str("0123456789ABCDEFGHJKMNPQRSTVWXYZ");
        spec.translate.from.push_str("abcdefghjkmnpqrstvwxyzOoIiLl");
        spec.translate.to.push_str("ABCDEFGHJKMNPQRSTVWXYZ001111");
        spec.ignore.push_str("-_ \t\r\n");
        // A failure here is a typo in the three literals above, caught by this
        // module's tests, and unreachable from any input.
        spec.encoding()
            .expect("the pairing-code alphabet is well-formed")
    })
}

/// BLAKE2s-256 over the concatenation of `parts`, via `snow`'s resolver.
fn blake2s(parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = DefaultResolver
        .resolve_hash(&HashChoice::Blake2s)
        .expect("snow's default resolver always provides BLAKE2s");
    hasher.reset();
    for part in parts {
        hasher.input(part);
    }
    let mut out = [0u8; 32];
    hasher.result(&mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::testutil::assert_no_secret;

    #[test]
    fn code_round_trips() {
        let token = PairingToken::generate();
        let code = token.to_code();
        let parsed = PairingToken::parse(&code).expect("own code must parse");
        assert_eq!(token, parsed);
        assert_eq!(token.psk(), parsed.psk());
        assert_eq!(token.pairing_id(), parsed.pairing_id());
    }

    #[test]
    fn code_shape_is_grouped_crockford_base32() {
        let token = PairingToken::from_bytes(&[0u8; TOKEN_LEN]);
        let code = token.to_code();
        // 32 bytes -> 52 base32 symbols -> 13 groups of 4, 12 separators.
        assert_eq!(code.len(), 52 + 12);
        let groups: Vec<&str> = code.split(CODE_SEPARATOR).collect();
        assert_eq!(groups.len(), 13);
        assert!(groups.iter().all(|g| g.len() == CODE_GROUP));
        // Crockford excludes these four letters entirely.
        for bad in ['I', 'L', 'O', 'U'] {
            assert!(
                !PairingToken::generate().to_code().contains(bad),
                "ambiguous glyph {bad} must not be emitted"
            );
        }
    }

    #[test]
    fn code_parses_lowercase_ungrouped_and_respaced() {
        let token = PairingToken::generate();
        let code = token.to_code();
        let bare: String = code.chars().filter(|c| *c != CODE_SEPARATOR).collect();

        for variant in [
            code.clone(),
            code.to_lowercase(),
            bare.clone(),
            bare.to_lowercase(),
            format!("  {code}  "),
            code.replace(CODE_SEPARATOR, " "),
            code.replace(CODE_SEPARATOR, "_"),
            bare.chars()
                .collect::<Vec<_>>()
                .chunks(8)
                .map(|c| c.iter().collect::<String>())
                .collect::<Vec<_>>()
                .join("\n"),
        ] {
            let parsed = PairingToken::parse(&variant)
                .unwrap_or_else(|_| panic!("variant must parse: {variant:?}"));
            assert_eq!(token, parsed, "variant changed the token: {variant:?}");
        }
    }

    #[test]
    fn code_parse_translates_ambiguous_glyphs() {
        // A user reading the code aloud will say "oh" for zero and "el" for
        // one. Both must land on the same token.
        let token = PairingToken::from_bytes(&[0u8; TOKEN_LEN]);
        let code = token.to_code(); // all zeroes -> all '0' symbols
        assert!(code.contains('0'));
        let mistyped = code.replace('0', "O");
        assert_eq!(
            PairingToken::parse(&mistyped).expect("O must translate to 0"),
            token
        );
        let one = PairingToken::from_bytes(&{
            let mut b = [0u8; TOKEN_LEN];
            // 0b00001000 00... -> second symbol is '1'
            b[0] = 0b0000_1000;
            b
        });
        let code = one.to_code();
        assert!(code.contains('1'));
        for glyph in ["I", "i", "L", "l"] {
            assert_eq!(
                PairingToken::parse(&code.replacen('1', glyph, 1))
                    .unwrap_or_else(|_| panic!("{glyph} must translate to 1")),
                one
            );
        }
    }

    #[test]
    fn code_parse_rejects_bad_input() {
        let good = PairingToken::generate().to_code();
        let cases = [
            String::new(),
            "not a pairing code".to_string(),
            good[..good.len() - 5].to_string(),        // too short
            format!("{good}ABCD"),                     // too long
            good.replacen(|c: char| c != '-', "U", 1), // U is not a symbol
            good.replacen(|c: char| c != '-', "$", 1), // outside the alphabet
        ];
        for case in cases {
            assert!(
                matches!(PairingToken::parse(&case), Err(TransportError::InvalidCode)),
                "must reject: {case:?}"
            );
        }
    }

    #[test]
    fn code_parse_rejects_non_canonical_trailing_bits() {
        // 52 symbols carry 260 bits; the last 4 must be zero. A code whose
        // trailing bits are set decodes to the same 32 bytes but is a second
        // valid spelling, which would make the pairing id ambiguous.
        let token = PairingToken::from_bytes(&[0u8; TOKEN_LEN]);
        let code = token.to_code();
        let bare: String = code.chars().filter(|c| *c != CODE_SEPARATOR).collect();
        let mut chars: Vec<char> = bare.chars().collect();
        chars[51] = '1'; // sets a trailing bit
        let tweaked: String = chars.into_iter().collect();
        assert!(matches!(
            PairingToken::parse(&tweaked),
            Err(TransportError::InvalidCode)
        ));
    }

    #[test]
    fn pairing_id_is_stable_derived_and_not_the_token() {
        let token = PairingToken::generate();
        let id = token.pairing_id();
        assert_eq!(id, token.pairing_id(), "must be deterministic");
        assert_eq!(id.len(), PAIRING_ID_LEN * 2);
        assert!(id
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()));

        // It must not be, or contain, the secret in any obvious encoding.
        let psk = token.psk();
        assert!(!id.contains(&hex::encode(psk)));
        assert!(!id.contains(&token.to_code()));
        assert_ne!(id.as_bytes(), &psk[..]);
        // Nor may it be the first half of the token.
        assert_ne!(id, hex::encode(&psk[..PAIRING_ID_LEN]));

        // Different tokens, different ids.
        let other = PairingToken::generate();
        assert_ne!(id, other.pairing_id());

        // A one-bit change in the token changes the id.
        let mut flipped = psk;
        flipped[0] ^= 1;
        assert_ne!(id, PairingToken::from_bytes(&flipped).pairing_id());
    }

    #[test]
    fn token_debug_never_prints_key_material() {
        let token = PairingToken::generate();
        let rendered = format!("{token:?}");
        assert_no_secret(&rendered, &token);
        assert!(rendered.contains(&token.pairing_id()));
    }

    /// Security review F-8. The contract is a type-level one (port manifest 02,
    /// I-12: "the tests that pin these are type-level"), so the guard against a
    /// regression is that this stops compiling — a `PskCandidate` that lost its
    /// `ZeroizeOnDrop` fails `assert_zeroize_on_drop`.
    #[test]
    fn a_psk_candidate_wipes_its_key() {
        fn assert_zeroize_on_drop<T: ZeroizeOnDrop>() {}
        assert_zeroize_on_drop::<PskCandidate>();

        let mut candidate = PskCandidate {
            pairing_id: "a-pairing".to_string(),
            psk: [7u8; TOKEN_LEN],
        };
        candidate.zeroize();
        assert_eq!(candidate.psk, [0u8; TOKEN_LEN]);
    }

    #[test]
    fn psk_candidate_debug_never_prints_key_material() {
        let token = PairingToken::generate();
        let candidate = PskCandidate {
            pairing_id: token.pairing_id(),
            psk: token.psk(),
        };
        let rendered = format!("{candidate:?}");
        assert_no_secret(&rendered, &token);
        assert!(!rendered.contains("psk"), "the key field must not appear");
        assert!(rendered.contains(&token.pairing_id()));
    }

    #[test]
    fn token_equality_is_by_value() {
        let a = PairingToken::generate();
        let b = PairingToken::from_bytes(&a.psk());
        assert_eq!(a, b);
        let mut different = a.psk();
        different[31] ^= 0x80;
        assert_ne!(a, PairingToken::from_bytes(&different));
    }
}
