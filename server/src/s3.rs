//! Accès au stockage objet compatible S3 (Cloudflare R2) pour la bibliothèque d'assets de POP Forge (#121, #91).
//! Liens signés AWS Signature V4 (paramètres dans l'adresse) : le navigateur envoie ou lit le fichier directement dans
//! le stockage, jamais par le serveur ; les clés restent sur le serveur.
//!
//! Variables : `POP_S3_ENDPOINT` (ex. https://<compte>.r2.cloudflarestorage.com), `POP_S3_BUCKET`, `POP_S3_REGION`
//! (auto pour R2), `POP_S3_KEY_ID`, `POP_S3_SECRET`.
// Utilisé par la bibliothèque d'assets (#91), fusionnée après ce module.
#![allow(dead_code)]
use crate::auth::sha256_hex;
use hmac::{Hmac, Mac};
use sha2::Sha256;

/// Accès au stockage objet.
pub struct S3 {
    endpoint: String,
    bucket: String,
    region: String,
    key_id: String,
    secret: String,
}

impl S3 {
    pub fn from_env() -> Option<S3> {
        let v = |k: &str| std::env::var(k).ok().filter(|s| !s.trim().is_empty());
        Some(S3 {
            endpoint: v("POP_S3_ENDPOINT")?.trim_end_matches('/').to_string(),
            bucket: v("POP_S3_BUCKET")?,
            region: v("POP_S3_REGION").unwrap_or_else(|| "auto".into()),
            key_id: v("POP_S3_KEY_ID")?,
            secret: v("POP_S3_SECRET")?,
        })
    }

    /// Lien signé (AWS Signature V4, paramètres dans l'adresse) pour `method` sur `key`, valable `secs` secondes.
    /// Adresse « par chemin » : <endpoint>/<bucket>/<key>, comme le demande R2.
    pub fn presign(
        &self,
        method: &str,
        key: &str,
        secs: u64,
        now: chrono::DateTime<chrono::Utc>,
    ) -> String {
        let (scheme, host) = self
            .endpoint
            .split_once("://")
            .unwrap_or(("https", &self.endpoint));
        let path = if self.bucket.is_empty() {
            format!("/{}", uri_encode(key, false))
        } else {
            format!(
                "/{}/{}",
                uri_encode(&self.bucket, false),
                uri_encode(key, false)
            )
        };
        presign_raw(
            method,
            scheme,
            host,
            &path,
            &self.region,
            &self.key_id,
            &self.secret,
            secs,
            now,
        )
    }
}

fn uri_encode(s: &str, slash: bool) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b'/' if !slash => out.push('/'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn hmac(key: &[u8], data: &str) -> Vec<u8> {
    let mut m = Hmac::<Sha256>::new_from_slice(key).expect("clé HMAC");
    m.update(data.as_bytes());
    m.finalize().into_bytes().to_vec()
}

#[allow(clippy::too_many_arguments)]
fn presign_raw(
    method: &str,
    scheme: &str,
    host: &str,
    path: &str,
    region: &str,
    key_id: &str,
    secret: &str,
    secs: u64,
    now: chrono::DateTime<chrono::Utc>,
) -> String {
    let date = now.format("%Y%m%d").to_string();
    let stamp = now.format("%Y%m%dT%H%M%SZ").to_string();
    let scope = format!("{date}/{region}/s3/aws4_request");
    let query = format!(
        "X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Credential={}&X-Amz-Date={stamp}&X-Amz-Expires={secs}&X-Amz-SignedHeaders=host",
        uri_encode(&format!("{key_id}/{scope}"), true)
    );
    let canonical = format!("{method}\n{path}\n{query}\nhost:{host}\n\nhost\nUNSIGNED-PAYLOAD");
    let to_sign = format!(
        "AWS4-HMAC-SHA256\n{stamp}\n{scope}\n{}",
        sha256_hex(&canonical)
    );
    let k = hmac(format!("AWS4{secret}").as_bytes(), &date);
    let k = hmac(&k, region);
    let k = hmac(&k, "s3");
    let k = hmac(&k, "aws4_request");
    let sig = hex::encode(hmac(&k, &to_sign));
    format!("{scheme}://{host}{path}?{query}&X-Amz-Signature={sig}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    /// Exemple officiel d'AWS (« Authenticating Requests: Using Query Parameters », GET de test.txt).
    #[test]
    fn signature_aws() {
        let t = chrono::Utc.with_ymd_and_hms(2013, 5, 24, 0, 0, 0).unwrap();
        let url = presign_raw(
            "GET",
            "https",
            "examplebucket.s3.amazonaws.com",
            "/test.txt",
            "us-east-1",
            "AKIAIOSFODNN7EXAMPLE",
            "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
            86400,
            t,
        );
        assert!(
            url.ends_with(
                "X-Amz-Signature=aeeed9bbccd4d02ee5c0109b86d86835f995330da4c265957d157751f604d404"
            ),
            "{url}"
        );
    }

    /// Lien vers R2 « par chemin », avec la signature mais jamais le secret.
    #[test]
    fn lien_r2() {
        let s3 = S3 {
            endpoint: "https://compte.r2.cloudflarestorage.com".into(),
            bucket: "pop-assets".into(),
            region: "auto".into(),
            key_id: "cle".into(),
            secret: "tres-secret".into(),
        };
        let t = chrono::Utc
            .with_ymd_and_hms(2026, 11, 21, 12, 0, 0)
            .unwrap();
        let url = s3.presign("PUT", "studios/1/abc/arbre vert.fbx", 900, t);
        assert!(url.starts_with("https://compte.r2.cloudflarestorage.com/pop-assets/studios/1/abc/arbre%20vert.fbx?"), "{url}");
        assert!(url.contains("X-Amz-Expires=900") && url.contains("X-Amz-Signature="));
        assert!(!url.contains("tres-secret"));
    }
}
