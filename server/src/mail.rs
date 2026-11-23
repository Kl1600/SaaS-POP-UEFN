//! Envoi d'e-mails par SMTP (POP_SMTP_HOST, POP_SMTP_PORT, POP_SMTP_USER, POP_SMTP_PASS, POP_SMTP_FROM).
//! Sans configuration, POP fonctionne quand même : les liens (invitation, réinitialisation) s'affichent à copier.
//! Chaque e-mail part en deux versions : le texte brut donné par l'appelant et une version HTML aux couleurs
//! de POP construite à partir de ce texte (#274).
use lettre::{
    message::MultiPart, transport::smtp::authentication::Credentials, AsyncSmtpTransport,
    AsyncTransport, Message, Tokio1Executor,
};

pub fn configured() -> bool {
    std::env::var("POP_SMTP_HOST")
        .map(|h| !h.is_empty())
        .unwrap_or(false)
}

/// Adresse publique de POP pour les liens des e-mails (POP_PUBLIC_URL, ex. https://pop.mon-studio.fr).
pub fn public_url() -> String {
    std::env::var("POP_PUBLIC_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8080".into())
        .trim_end_matches('/')
        .to_string()
}

pub async fn send(to: &str, subject: &str, body: &str) -> anyhow::Result<()> {
    let host = std::env::var("POP_SMTP_HOST")?;
    let port: u16 = std::env::var("POP_SMTP_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(587);
    let from = std::env::var("POP_SMTP_FROM").unwrap_or_else(|_| format!("POP <no-reply@{host}>"));
    let html = render_html(subject, body, &public_url());
    let msg = Message::builder()
        .from(from.parse()?)
        .to(to.parse()?)
        .subject(subject)
        .multipart(MultiPart::alternative_plain_html(body.to_string(), html))?;
    let mut b = if port == 465 {
        AsyncSmtpTransport::<Tokio1Executor>::relay(&host)?
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&host)?
    }
    .port(port);
    if let (Ok(u), Ok(p)) = (
        std::env::var("POP_SMTP_USER"),
        std::env::var("POP_SMTP_PASS"),
    ) {
        b = b.credentials(Credentials::new(u, p));
    }
    b.build().send(msg).await?;
    Ok(())
}

const ACCENT: &str = "#ED753E";

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            _ => o.push(c),
        }
    }
    o
}

fn is_url(s: &str) -> bool {
    (s.starts_with("https://") || s.starts_with("http://")) && !s.contains(char::is_whitespace)
}

/// Libellé du bouton selon la page visée par le lien.
fn button_label(url: &str) -> &'static str {
    let path = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    let path = path.find('/').map(|i| &path[i..]).unwrap_or("/");
    let rules: [(&str, &str); 7] = [
        ("/verifier-email", "Confirmer mon adresse"),
        ("/reinitialiser", "Choisir un nouveau mot de passe"),
        ("/activer", "Activer mon compte"),
        ("/invitation", "Rejoindre le studio"),
        ("/inscription", "Créer mon compte"),
        ("/connexion", "Se connecter"),
        ("/admin", "Ouvrir l’administration"),
    ];
    rules
        .iter()
        .find(|(p, _)| path.starts_with(p))
        .map(|(_, l)| *l)
        .unwrap_or("Ouvrir POP")
}

/// Une ligne de texte : échappée, les adresses web deviennent des liens.
fn inline(line: &str) -> String {
    line.split(' ')
        .map(|w| {
            let (core, tail) = match w.strip_suffix(['.', ',', ')']) {
                Some(c) if is_url(c) => (c, &w[c.len()..]),
                _ => (w, ""),
            };
            if is_url(core) {
                format!(
                    "<a href=\"{u}\" style=\"color:{ACCENT};text-decoration:underline;word-break:break-all\">{u}</a>{}",
                    esc(tail),
                    u = esc(core)
                )
            } else {
                esc(w)
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn button(url: &str, primary: bool) -> String {
    let (bg, fg, border) = if primary {
        (ACCENT, "#ffffff", ACCENT)
    } else {
        ("#ffffff", "#111111", "#d4d4d4")
    };
    format!(
        "<table role=\"presentation\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\" style=\"margin:8px 0 20px\"><tr><td style=\"border-radius:10px;background:{bg};border:1px solid {border}\">\
<a href=\"{u}\" style=\"display:inline-block;padding:13px 24px;font-size:15px;font-weight:600;color:{fg};text-decoration:none;border-radius:10px\">{label}</a>\
</td></tr></table>",
        u = esc(url),
        label = esc(button_label(url))
    )
}

const P: &str = "margin:0 0 16px;font-size:15px;line-height:1.6;color:#222222";

/// Construit la version HTML d'un e-mail à partir de son texte brut.
/// Les paragraphes sont séparés par une ligne vide ; une adresse seule sur sa ligne devient un bouton
/// (le premier en orange), avec l'adresse en clair en bas pour la copier ; la signature est en gris.
pub fn render_html(subject: &str, body: &str, site: &str) -> String {
    let body = body.replace("\r\n", "\n");
    let mut content = String::new();
    let mut links: Vec<String> = vec![];
    let mut preheader = String::new();
    for (i, para) in body.split("\n\n").enumerate() {
        let lines: Vec<&str> = para
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        if lines.is_empty() {
            continue;
        }
        if i > 0 && preheader.is_empty() && !is_url(lines[0]) {
            preheader = lines.join(" ");
        }
        let signature = lines
            .iter()
            .any(|l| l.contains("L’équipe POP") || l.contains("L'équipe POP"));
        let mut text: Vec<String> = vec![];
        let flush = |text: &mut Vec<String>, content: &mut String| {
            if !text.is_empty() {
                let style = if signature {
                    "margin:0 0 16px;font-size:14px;line-height:1.6;color:#737373"
                } else {
                    P
                };
                content.push_str(&format!("<p style=\"{style}\">{}</p>", text.join("<br>")));
                text.clear();
            }
        };
        for l in &lines {
            if is_url(l) {
                flush(&mut text, &mut content);
                content.push_str(&button(l, links.is_empty()));
                links.push(l.to_string());
            } else {
                text.push(inline(l));
            }
        }
        flush(&mut text, &mut content);
    }
    let fallback = if links.is_empty() {
        String::new()
    } else {
        let list = links
            .iter()
            .map(|l| {
                format!(
                    "<a href=\"{u}\" style=\"color:#737373;word-break:break-all\">{u}</a>",
                    u = esc(l)
                )
            })
            .collect::<Vec<_>>()
            .join("<br>");
        format!(
            "<tr><td style=\"padding:0 36px 28px\"><p style=\"margin:0;padding-top:18px;border-top:1px solid #ececec;font-size:12px;line-height:1.6;color:#8a8a8a\">\
Le bouton ne fonctionne pas ? Copiez ce lien dans votre navigateur :<br>{list}</p></td></tr>"
        )
    };
    let preheader: String = preheader.chars().take(140).collect();
    let site_label = site
        .split_once("://")
        .map(|(_, r)| r)
        .unwrap_or(site)
        .to_string();
    format!(
        "<!DOCTYPE html><html lang=\"fr\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
<meta name=\"color-scheme\" content=\"light\"><meta name=\"supported-color-schemes\" content=\"light\"><title>{title}</title></head>\
<body style=\"margin:0;padding:0;background:#f4f4f5;-webkit-text-size-adjust:100%\">\
<div style=\"display:none;max-height:0;overflow:hidden;opacity:0;color:transparent\">{pre}</div>\
<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\" style=\"background:#f4f4f5\"><tr><td align=\"center\" style=\"padding:32px 12px\">\
<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\" style=\"max-width:560px;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,Arial,sans-serif\">\
<tr><td style=\"background:#000000;border-radius:16px 16px 0 0;padding:22px 36px\">\
<a href=\"{site}\" style=\"text-decoration:none\"><img src=\"{site}/logo.png\" width=\"54\" height=\"37\" alt=\"POP\" style=\"display:block;border:0;color:#ffffff;font-size:22px;font-weight:700\"></a></td></tr>\
<tr><td style=\"background:#ffffff;border:1px solid #e5e5e5;border-top:0;border-radius:0 0 16px 16px\">\
<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\">\
<tr><td style=\"padding:32px 36px 12px\"><h1 style=\"margin:0 0 20px;font-size:21px;line-height:1.35;font-weight:600;color:#111111\">{title}</h1>{content}</td></tr>\
{fallback}</table></td></tr>\
<tr><td style=\"padding:20px 36px;text-align:center;font-size:12px;line-height:1.6;color:#8a8a8a\">\
POP · l’espace de travail des créateurs UEFN<br><a href=\"{site}\" style=\"color:#8a8a8a\">{site_label}</a><br>\
E-mail automatique : merci de ne pas y répondre.</td></tr>\
</table></td></tr></table></body></html>",
        title = esc(&title_of(subject)),
        pre = esc(&preheader),
        site = esc(site),
        site_label = esc(&site_label),
    )
}

/// Titre affiché dans l'e-mail : l'objet sans le préfixe « POP · » ou « POP : », avec une majuscule.
fn title_of(subject: &str) -> String {
    let t = subject
        .strip_prefix("POP · ")
        .or_else(|| subject.strip_prefix("POP : "))
        .unwrap_or(subject);
    let mut c = t.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests_mail {
    use super::*;

    #[test]
    fn bouton_et_lien_de_secours() {
        let h = render_html(
            "POP · réinitialiser votre mot de passe",
            "Bonjour,\n\nPour choisir un nouveau mot de passe, ouvrez ce lien :\nhttps://pop-uefn.com/reinitialiser/abc\n\nL’équipe POP",
            "https://pop-uefn.com",
        );
        assert!(h.contains(">Réinitialiser votre mot de passe</h1>"));
        assert!(h.contains(">Choisir un nouveau mot de passe</a>"));
        assert!(h.contains("Copiez ce lien"));
        assert!(h.contains("https://pop-uefn.com/logo.png"));
        assert!(h.contains("color:#737373\">L’équipe POP"));
    }

    #[test]
    fn texte_echappe() {
        let h = render_html(
            "<b>x</b> vous invite",
            "Bonjour,\n\n<script>alert(1)</script> vous invite dans \"Studio & co\".\n\nRejoindre : https://pop-uefn.com/invitation/t",
            "https://pop-uefn.com",
        );
        assert!(!h.contains("<script>"));
        assert!(!h.contains("<b>x"));
        assert!(h.contains("&lt;script&gt;"));
        assert!(h.contains("Studio &amp; co"));
        assert!(h.contains("href=\"https://pop-uefn.com/invitation/t\""));
    }

    #[test]
    fn libelles() {
        assert_eq!(
            button_label("https://x.fr/verifier-email/t"),
            "Confirmer mon adresse"
        );
        assert_eq!(
            button_label("https://x.fr/activer?token=t"),
            "Activer mon compte"
        );
        assert_eq!(
            button_label("https://x.fr/invitation/t"),
            "Rejoindre le studio"
        );
        assert_eq!(
            button_label("https://x.fr/inscription?acces=t"),
            "Créer mon compte"
        );
        assert_eq!(button_label("https://x.fr/"), "Ouvrir POP");
    }
}
