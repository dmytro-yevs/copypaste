//! Eight-symbol, short-lived passwords for the authenticated pairing exchange.

use rand::{rngs::OsRng, RngCore};
use zeroize::{Zeroize, Zeroizing};

use super::{token::code_encoding, TransportError};

const CODE_BYTES: usize = 5;

/// A random Crockford code. It is never used as a stored Noise PSK.
pub struct PairingCode(Zeroizing<[u8; CODE_BYTES]>);

impl PairingCode {
    #[must_use]
    pub fn generate() -> Self {
        let mut bytes = Zeroizing::new([0; CODE_BYTES]);
        OsRng.fill_bytes(bytes.as_mut_slice());
        Self(bytes)
    }

    pub fn parse(code: &str) -> Result<Self, TransportError> {
        let mut decoded = code_encoding()
            .decode(code.as_bytes())
            .map_err(|_| TransportError::InvalidCode)?;
        let result = decoded
            .as_slice()
            .try_into()
            .map(|bytes| Self(Zeroizing::new(bytes)))
            .map_err(|_| TransportError::InvalidCode);
        decoded.zeroize();
        result
    }

    #[must_use]
    pub fn to_code(&self) -> String {
        code_encoding().encode(self.0.as_slice())
    }

    pub(super) fn password(&self) -> &[u8] {
        self.0.as_slice()
    }

    pub(crate) fn copy_secret(&self) -> Self {
        Self(Zeroizing::new(*self.0))
    }
}

impl std::fmt::Debug for PairingCode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("PairingCode(<redacted>)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eight_symbols_round_trip_with_human_formatting() {
        let code = PairingCode::generate().to_code();
        assert_eq!(code.len(), 8);
        for text in [
            code.to_lowercase(),
            format!("{}-{}", &code[..4], &code[4..]),
        ] {
            assert_eq!(PairingCode::parse(&text).unwrap().to_code(), code);
        }
        assert!(!format!("{:?}", PairingCode::parse(&code).unwrap()).contains(&code));
        for invalid in ["", "1234567", "123456789", "UUUUUUUU"] {
            assert!(PairingCode::parse(invalid).is_err());
        }
    }
}
