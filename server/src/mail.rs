//! Envoi d'e-mails par SMTP (POP_SMTP_HOST, POP_SMTP_PORT, POP_SMTP_USER, POP_SMTP_PASS, POP_SMTP_FROM).
//! Sans configuration, POP fonctionne quand même : les liens (invitation, réinitialisation) s'affichent à copier.
use lettre::{message::header::ContentType, transport::smtp::authentication::Credentials, AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

pub fn configured() -> bool {
    std::env::var("POP_SMTP_HOST").map(|h| !h.is_empty()).unwrap_or(false)
}

/// Adresse publique de POP pour les liens des e-mails (POP_PUBLIC_URL, ex. https://pop.mon-studio.fr).
pub fn public_url() -> String {
    std::env::var("POP_PUBLIC_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".into()).trim_end_matches('/').to_string()
}

pub async fn send(to: &str, subject: &str, body: &str) -> anyhow::Result<()> {
    let host = std::env::var("POP_SMTP_HOST")?;
    let port: u16 = std::env::var("POP_SMTP_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(587);
    let from = std::env::var("POP_SMTP_FROM").unwrap_or_else(|_| format!("POP <no-reply@{host}>"));
    let msg = Message::builder().from(from.parse()?).to(to.parse()?).subject(subject).header(ContentType::TEXT_PLAIN).body(body.to_string())?;
    let mut b = if port == 465 { AsyncSmtpTransport::<Tokio1Executor>::relay(&host)? } else { AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&host)? }.port(port);
    if let (Ok(u), Ok(p)) = (std::env::var("POP_SMTP_USER"), std::env::var("POP_SMTP_PASS")) {
        b = b.credentials(Credentials::new(u, p));
    }
    b.build().send(msg).await?;
    Ok(())
}
