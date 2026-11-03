use crate::errors::LauncherError;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};

/// Публичный ключ задаётся при сборке релиза. Без него подпись манифестов не проверяется.
const EMBEDDED_PUBLIC_KEY: Option<&str> = option_env!("FENIX_MANIFEST_PUBLIC_KEY");

pub fn embedded_public_key() -> Option<String> {
    EMBEDDED_PUBLIC_KEY
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(str::to_string)
}

fn invalid(reason: &str) -> LauncherError {
    LauncherError::BadManifest(reason.to_string())
}

/// Проверяет подпись точных байтов ответа сервера.
pub fn verify_signature(
    public_key: &str,
    body: &[u8],
    signature: &str,
) -> Result<(), LauncherError> {
    let key_bytes: [u8; 32] = STANDARD
        .decode(public_key.trim())
        .ok()
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| invalid("в лаунчере указан неверный ключ проверки"))?;
    let key = VerifyingKey::from_bytes(&key_bytes)
        .map_err(|_| invalid("в лаунчере указан неверный ключ проверки"))?;
    let signature_bytes: [u8; 64] = STANDARD
        .decode(signature.trim())
        .ok()
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| invalid("подпись имеет неверный формат"))?;
    key.verify(body, &Signature::from_bytes(&signature_bytes))
        .map_err(|_| invalid("подпись не совпадает"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Этот же вектор проверяется в backend/tests/test_signing_and_feeds.py.
    const PUBLIC_KEY: &str = "6kpsY+KcUgq+9VB7Ey7F+ZVHdq6+vnuSQh7qaRRG0iw=";
    const BODY: &[u8] = br#"{"files":[],"profile":"interop"}"#;
    const SIGNATURE: &str =
        "JVYo3kKBG2N6VW2EDrkNeZdD6AmndO0+bObqfqfcAerxkS3DofmXPe9EKmeR8Xabu80D/v30+u16MQ8eoxFfDA==";

    #[test]
    fn signature_made_by_the_backend_is_accepted() {
        assert_eq!(verify_signature(PUBLIC_KEY, BODY, SIGNATURE), Ok(()));
    }

    #[test]
    fn changed_body_is_rejected() {
        let tampered = br#"{"files":[],"profile":"interoq"}"#;
        assert!(matches!(
            verify_signature(PUBLIC_KEY, tampered, SIGNATURE),
            Err(LauncherError::BadManifest(_))
        ));
    }

    #[test]
    fn malformed_values_are_rejected() {
        for (key, signature) in [
            (PUBLIC_KEY, "not base64!"),
            (PUBLIC_KEY, "AAAA"),
            ("not base64!", SIGNATURE),
            ("AAAA", SIGNATURE),
        ] {
            assert!(
                matches!(
                    verify_signature(key, BODY, signature),
                    Err(LauncherError::BadManifest(_))
                ),
                "accepted {key} / {signature}"
            );
        }
    }
}
