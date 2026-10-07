//! Incoming SMS recognition is local and stateless. Message bodies are never
//! written to module storage, returned as results, or sent over the network.
use copypaste_module_sdk::{Module, ModuleEnvironment, ModuleInvocation, ModuleOutput};
use regex::Regex;
use std::collections::BTreeSet;

pub struct SmsCodes {
    detector: CodeDetector,
}
impl Module for SmsCodes {
    fn create(_: ModuleEnvironment) -> Result<Self, String> {
        Ok(Self {
            detector: CodeDetector::new()?,
        })
    }
    fn invoke(&mut self, invocation: ModuleInvocation) -> Result<ModuleOutput, String> {
        if invocation.command != "extract-code" {
            return Err("Unknown SMS command.".into());
        }
        let text = invocation
            .arguments
            .get("text")
            .and_then(|value| value.as_str())
            .ok_or("SMS text is required.")?;
        Ok(ModuleOutput::Text {
            text: self.detector.extract_code(text).unwrap_or_default(),
        })
    }
}
copypaste_module_sdk::export_module!(SmsCodes);

struct CodeDetector {
    context: Regex,
    promotion: Regex,
    candidate: Regex,
}
impl CodeDetector {
    fn new() -> Result<Self, String> {
        Ok(Self {
            context: Regex::new(r"(?i)\b(?:otp|pin|code|passcode|password|verification|verify|authentication|authorization|confirmation|код|пароль|підтвердження|подтверждения|código|codigo|kennwort|passwort|tan)\b").map_err(|_| "Invalid authentication pattern.")?,
            promotion: Regex::new(r"(?i)\b(?:promo(?:tion)?(?:code)?|coupon|discount|voucher|промокод|промо|знижк\w*|скидк\w*|купон|посилк\w*|посылк\w*|parcel|tracking|delivery|pickup|order)\b").map_err(|_| "Invalid unrelated code pattern.")?,
            candidate: Regex::new(r"[0-9]{3,4}[ -][0-9]{3,4}|[A-Za-z0-9]+").map_err(|_| "Invalid code candidate pattern.")?,
        })
    }

    /// Keep leading zeroes and case. Equally plausible distinct codes are skipped
    /// instead of silently selecting a number such as a price or expiry date.
    fn extract_code(&self, text: &str) -> Option<String> {
        if text.is_empty() || text.len() > 64 * 1024 {
            return None;
        }
        let contexts: Vec<_> = self.context.find_iter(text).collect();
        if contexts.is_empty() {
            return None;
        }
        let promotions: Vec<_> = self.promotion.find_iter(text).collect();
        if !promotions.is_empty() {
            return None;
        }
        let mut best = BTreeSet::new();
        for candidate in self.candidate.find_iter(text) {
            let raw = candidate.as_str();
            if text[..candidate.start()]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric() || "/@.:".contains(c))
                || text[candidate.end()..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_alphanumeric() || "/@:".contains(c))
            {
                continue;
            }
            let code: String = raw.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
            if !(4..=10).contains(&code.len())
                || (!code.bytes().any(|c| c.is_ascii_digit())
                    && !code.bytes().all(|c| c.is_ascii_uppercase()))
                || contexts.iter().any(|context| {
                    context.start() < candidate.end() && context.end() > candidate.start()
                })
            {
                continue;
            }
            // Phone numbers, years in dates, and decimal amounts aren't codes.
            if text[..candidate.start()].ends_with('+')
                || text[candidate.end()..].starts_with(['-', '/'])
                || text[candidate.end()..].starts_with('.')
                    && text[candidate.end() + 1..]
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_ascii_digit())
            {
                continue;
            }
            let distance = |start: usize, end: usize| {
                if end <= candidate.start() {
                    candidate.start() - end
                } else if start >= candidate.end() {
                    start - candidate.end()
                } else {
                    usize::MAX
                }
            };
            let score = contexts
                .iter()
                .map(|context| distance(context.start(), context.end()))
                .min()?;
            if score > 64 {
                continue;
            }
            best.insert(code);
        }
        (best.len() == 1).then(|| best.into_iter().next()).flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_module_sdk::serde_json;
    fn extract_code(text: &str) -> Option<String> {
        CodeDetector::new().unwrap().extract_code(text)
    }
    #[test]
    fn native_abi_returns_only_the_code_and_releases_owned_state() {
        // SAFETY: the exported static table is valid for this process. Inputs
        // remain borrowed through each call, calls are serialized, and every
        // native reply and the instance are released exactly once by their owner.
        unsafe {
            let api = &*copypaste_module_v1();
            let environment = br#"{"package_dir":"/module","data_dir":"/data"}"#;
            let instance = (api.create)(environment.as_ptr(), environment.len());
            assert!(!instance.is_null());
            for (sms, code) in [
                ("Ваш код підтвердження: 007123", "007123"),
                ("Your promo code is SAVE2026", ""),
            ] {
                let input = serde_json::to_vec(&serde_json::json!({"command":"extract-code", "arguments":{"text":sms}, "preferences":{}})).unwrap();
                let reply = (api.invoke)(instance, input.as_ptr(), input.len());
                let bytes =
                    std::slice::from_raw_parts(reply.buffer.data, reply.buffer.len).to_vec();
                (api.release)(reply.buffer);
                assert_eq!(reply.status, 0);
                assert_eq!(
                    serde_json::from_slice::<ModuleOutput>(&bytes).unwrap(),
                    ModuleOutput::Text { text: code.into() }
                );
            }
            (api.destroy)(instance);
        }
    }
    #[test]
    fn recognizes_login_confirmation_and_transaction_codes() {
        for (sms, code) in [
            (
                "Your verification code is 007123. Expires in 5 minutes.",
                "007123",
            ),
            (
                "Ваш код підтвердження: 123456. Не повідомляйте його нікому.",
                "123456",
            ),
            ("Код для підтвердження операції 9452", "9452"),
            ("OTP: aB12Cd", "aB12Cd"),
            ("OTP: ABCDEF", "ABCDEF"),
            ("PIN 9281", "9281"),
            ("Use 123456 to verify your account", "123456"),
            ("Your code: 123-456", "123456"),
            ("Your code: 123 456", "123456"),
            ("Your code: 654321\nFA+9qCX9VSu", "654321"),
        ] {
            assert_eq!(extract_code(sms).as_deref(), Some(code), "{sms}");
        }
    }
    #[test]
    fn skips_unrelated_numbers_promotions_and_ambiguous_codes() {
        for sms in [
            "Order 123456 has shipped",
            "Your promo code is SAVE2026",
            "Промокод SALE2026",
            "Parcel pickup code: 123456",
            "Код отримання посилки: 123456",
            "Balance: 1234.56",
            "Call +380501234567 to get your code",
            "Your code expires on 2026-10-07",
            "OTP: 1234 or 5678",
            "Hello world",
            "https://example.com/code/123456",
        ] {
            assert_eq!(extract_code(sms), None, "{sms}");
        }
    }
}
