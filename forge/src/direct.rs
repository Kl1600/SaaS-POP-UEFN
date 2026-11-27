//! Terminal en direct dans Forge (#466) : Forge fait tourner Claude Code sur ce PC (avec l'abonnement de
//! l'utilisateur), en mode conversation par flux JSON. Ce qu'on écrit dans le terminal de l'agent part tout de suite
//! dans son entrée ; ce qu'il répond et les outils qu'il appelle s'affichent au fil de l'eau. Aucune fenêtre de
//! console ; la clé POP passe par `POP_TOKEN` (jamais dans un fichier). Fermer le terminal ou Forge arrête l'outil.
use serde::Serialize;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};

/// Lignes gardées par terminal (les plus anciennes partent).
const MAX_LIGNES: usize = 3000;

struct Session {
    entree: ChildStdin,
    process: Child,
    lignes: Arc<Mutex<(usize, Vec<String>)>>,
}

#[derive(Default)]
pub struct Directs(Mutex<HashMap<i64, Session>>);

impl Directs {
    /// Arrête tous les outils (fermeture de Forge).
    pub fn tout_arreter(&self) {
        for (_, mut s) in self.0.lock().unwrap().drain() {
            let _ = s.process.kill();
        }
    }
}

/// Arguments de Claude Code en mode conversation par flux : la consigne de départ en invite système, les outils POP
/// autorisés (serveur MCP « pop »), le reste demande l'autorisation habituelle de Claude Code.
pub fn arguments_claude(config: &Path, consigne: &str) -> Vec<String> {
    vec![
        "-p".into(),
        "--input-format".into(),
        "stream-json".into(),
        "--output-format".into(),
        "stream-json".into(),
        "--verbose".into(),
        "--mcp-config".into(),
        config.to_string_lossy().into(),
        "--allowedTools".into(),
        "mcp__pop".into(),
        "--append-system-prompt".into(),
        consigne.into(),
    ]
}

/// Message de l'humain au format d'entrée de Claude Code.
pub fn message(texte: &str) -> String {
    serde_json::json!({ "type": "user", "message": { "role": "user", "content": [{ "type": "text", "text": texte }] } })
        .to_string()
}

fn ajouter(l: &Arc<Mutex<(usize, Vec<String>)>>, ligne: String) {
    let mut g = l.lock().unwrap();
    g.1.push(ligne);
    if g.1.len() > MAX_LIGNES {
        let trop = g.1.len() - MAX_LIGNES;
        g.1.drain(..trop);
        g.0 += trop;
    }
}

pub fn ouvrir(
    d: &Directs,
    agent: i64,
    exe: &Path,
    args: &[String],
    dossier: &Path,
    token: &str,
) -> Result<(), String> {
    let mut sessions = d.0.lock().unwrap();
    if let Some(s) = sessions.get_mut(&agent) {
        if s.process.try_wait().ok().flatten().is_none() {
            return Ok(());
        }
    }
    // Un .cmd (installation npm) passe par cmd /c ; un .exe se lance directement.
    let mut c = if exe
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("cmd"))
    {
        let mut c = Command::new("cmd");
        c.arg("/c").arg(exe);
        c
    } else {
        Command::new(exe)
    };
    c.args(args)
        .current_dir(dossier)
        .env("POP_TOKEN", token)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const SANS_FENETRE: u32 = 0x0800_0000;
        c.creation_flags(SANS_FENETRE);
    }
    let mut p = c
        .spawn()
        .map_err(|e| format!("Lancement impossible : {e}"))?;
    let lignes = Arc::new(Mutex::new((0usize, Vec::new())));
    let sortie = p.stdout.take().ok_or("Sortie indisponible.")?;
    let erreurs = p.stderr.take().ok_or("Sortie d'erreur indisponible.")?;
    let l1 = lignes.clone();
    std::thread::spawn(move || {
        for l in BufReader::new(sortie).lines().map_while(Result::ok) {
            if !l.trim().is_empty() {
                ajouter(&l1, l);
            }
        }
        ajouter(&l1, serde_json::json!({ "type": "fin" }).to_string());
    });
    let l2 = lignes.clone();
    std::thread::spawn(move || {
        for l in BufReader::new(erreurs).lines().map_while(Result::ok) {
            if !l.trim().is_empty() {
                ajouter(
                    &l2,
                    serde_json::json!({ "type": "erreur", "text": l }).to_string(),
                );
            }
        }
    });
    let entree = p.stdin.take().ok_or("Entrée indisponible.")?;
    sessions.insert(
        agent,
        Session {
            entree,
            process: p,
            lignes,
        },
    );
    Ok(())
}

pub fn ecrire(d: &Directs, agent: i64, texte: &str) -> Result<(), String> {
    let mut sessions = d.0.lock().unwrap();
    let s = sessions
        .get_mut(&agent)
        .ok_or("Ce terminal n'est plus ouvert : rouvrez-le avec « + Terminal ».")?;
    let ligne = message(texte);
    writeln!(s.entree, "{ligne}")
        .and_then(|_| s.entree.flush())
        .map_err(|_| "L'outil s'est arrêté : rouvrez-le avec « + Terminal ».".to_string())?;
    ajouter(
        &s.lignes,
        serde_json::json!({ "type": "moi", "text": texte }).to_string(),
    );
    Ok(())
}

#[derive(Serialize)]
pub struct Lecture {
    lignes: Vec<String>,
    /// Position à redonner au prochain appel.
    suite: usize,
    vivant: bool,
}

/// Lignes reçues depuis `depuis` (position absolue).
pub fn lire(d: &Directs, agent: i64, depuis: usize) -> Option<Lecture> {
    let mut sessions = d.0.lock().unwrap();
    let s = sessions.get_mut(&agent)?;
    let vivant = s.process.try_wait().ok().flatten().is_none();
    let g = s.lignes.lock().unwrap();
    let debut = depuis.max(g.0) - g.0;
    Some(Lecture {
        lignes: g.1.get(debut.min(g.1.len())..).unwrap_or_default().to_vec(),
        suite: g.0 + g.1.len(),
        vivant,
    })
}

pub fn fermer(d: &Directs, agent: i64) {
    if let Some(mut s) = d.0.lock().unwrap().remove(&agent) {
        let _ = s.process.kill();
    }
}

pub fn actifs(d: &Directs) -> Vec<i64> {
    let mut sessions = d.0.lock().unwrap();
    let mut v = vec![];
    for (a, s) in sessions.iter_mut() {
        if s.process.try_wait().ok().flatten().is_none() {
            v.push(*a);
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_et_arguments() {
        let m: serde_json::Value = serde_json::from_str(&message("dis \"bonjour\"")).unwrap();
        assert_eq!(m["type"], "user");
        assert_eq!(m["message"]["content"][0]["text"], "dis \"bonjour\"");
        let a = arguments_claude(Path::new("c.json"), "Consigne");
        let pos = |x: &str| a.iter().position(|y| y == x).unwrap();
        assert_eq!(a[pos("--input-format") + 1], "stream-json");
        assert_eq!(a[pos("--output-format") + 1], "stream-json");
        assert_eq!(a[pos("--mcp-config") + 1], "c.json");
        assert_eq!(a[pos("--append-system-prompt") + 1], "Consigne");
    }

    /// Un faux outil (cat sous Unix) : ce qu'on écrit ressort, dans l'ordre, et la session s'arrête à la fermeture.
    #[cfg(unix)]
    #[test]
    fn aller_retour() {
        let d = Directs::default();
        ouvrir(
            &d,
            7,
            Path::new("/bin/cat"),
            &[],
            Path::new("/tmp"),
            "pop_x",
        )
        .unwrap();
        ecrire(&d, 7, "bonjour").unwrap();
        let mut l = vec![];
        for _ in 0..50 {
            l = lire(&d, 7, 0).unwrap().lignes;
            if l.len() >= 2 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(l.iter().any(|x| x.contains("\"moi\"")), "{l:?}");
        assert!(
            l.iter()
                .any(|x| x.contains("\"user\"") && x.contains("bonjour")),
            "{l:?}"
        );
        let r = lire(&d, 7, 0).unwrap();
        assert!(r.vivant && lire(&d, 7, r.suite).unwrap().lignes.is_empty());
        assert_eq!(actifs(&d), [7]);
        fermer(&d, 7);
        assert!(lire(&d, 7, 0).is_none() && actifs(&d).is_empty());
    }
}
