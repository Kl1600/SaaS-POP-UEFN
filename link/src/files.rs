//! Fichiers du projet UEFN : lecture et écriture des .verse, strictement confinées au dossier du projet.
use anyhow::{anyhow, bail, Result};
use std::{
    collections::HashMap,
    path::{Component, Path, PathBuf},
    time::SystemTime,
};

const SKIP: [&str; 7] = ["Intermediate", "Saved", "Binaries", "DerivedDataCache", ".urc", ".git", "node_modules"];

fn walk(root: &Path, dir: &Path, out: &mut Vec<PathBuf>, digests: bool) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        if ft.is_dir() {
            if !SKIP.contains(&name.as_str()) && !name.starts_with("__External") && out.len() < 5000 {
                walk(root, &p, out, digests);
            }
        } else if name.ends_with(".verse") && (name.ends_with(".digest.verse") == digests) {
            out.push(p);
        }
    }
}

pub fn rel(root: &Path, p: &Path) -> String {
    p.strip_prefix(root).unwrap_or(p).components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect::<Vec<_>>().join("/")
}

/// Résout un chemin relatif venant de POP en refusant toute sortie du projet.
pub fn resolve(root: &Path, relp: &str) -> Result<PathBuf> {
    if relp.is_empty() || relp.contains('\\') || relp.contains(':') || !relp.ends_with(".verse") || relp.ends_with(".digest.verse") {
        bail!("chemin refusé : {relp}");
    }
    let p = Path::new(relp);
    if p.components().any(|c| !matches!(c, Component::Normal(_))) {
        bail!("chemin refusé : {relp}");
    }
    let full = root.join(p);
    // Le dossier parent existant le plus proche doit rester dans le projet (pas de lien symbolique qui sort).
    let root_c = root.canonicalize()?;
    let mut probe = full.parent().map(Path::to_path_buf).ok_or_else(|| anyhow!("chemin invalide"))?;
    while !probe.exists() {
        probe = probe.parent().map(Path::to_path_buf).ok_or_else(|| anyhow!("chemin invalide"))?;
    }
    if !probe.canonicalize()?.starts_with(&root_c) {
        bail!("chemin hors du projet : {relp}");
    }
    Ok(full)
}

pub struct Scanner {
    pub root: PathBuf,
    seen: HashMap<String, (SystemTime, u64)>,
    digest_seen: HashMap<String, (SystemTime, u64)>,
}

impl Scanner {
    pub fn new(root: PathBuf) -> Self {
        Self { root, seen: HashMap::new(), digest_seen: HashMap::new() }
    }

    /// Fichiers .verse modifiés depuis le dernier passage (ou tous si `full`).
    pub fn changed(&mut self, full: bool) -> Vec<(String, String)> {
        let mut list = vec![];
        walk(&self.root, &self.root, &mut list, false);
        let mut out = vec![];
        let mut now = HashMap::new();
        for p in list {
            let Ok(md) = std::fs::metadata(&p) else { continue };
            if md.len() > 2_000_000 {
                continue;
            }
            let key = rel(&self.root, &p);
            let sig = (md.modified().unwrap_or(SystemTime::UNIX_EPOCH), md.len());
            if full || self.seen.get(&key) != Some(&sig) {
                if let Ok(c) = std::fs::read_to_string(&p) {
                    out.push((key.clone(), c));
                }
            }
            now.insert(key, sig);
        }
        let removed = self.seen.keys().any(|k| !now.contains_key(k));
        self.seen = now;
        if removed && !full {
            // Un fichier a disparu : on renverra la liste complète.
            return self.changed(true);
        }
        out
    }

    pub fn forget(&mut self, key: &str) {
        self.seen.remove(key);
    }

    /// Contenu des digests s'ils ont changé.
    pub fn digests(&mut self, extra: Option<&Path>) -> Option<Vec<(String, String)>> {
        let mut list = vec![];
        walk(&self.root, &self.root, &mut list, true);
        if let Some(d) = extra {
            walk(d, d, &mut list, true);
        }
        let mut now = HashMap::new();
        for p in &list {
            if let Ok(md) = std::fs::metadata(p) {
                now.insert(p.to_string_lossy().to_string(), (md.modified().unwrap_or(SystemTime::UNIX_EPOCH), md.len()));
            }
        }
        if now == self.digest_seen {
            return None;
        }
        self.digest_seen = now;
        Some(list.iter().filter_map(|p| std::fs::read_to_string(p).ok().map(|c| (p.file_name().unwrap_or_default().to_string_lossy().to_string(), c))).collect())
    }

    pub fn write(&mut self, relp: &str, content: &str) -> Result<()> {
        let p = resolve(&self.root, relp)?;
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d)?;
        }
        std::fs::write(&p, content)?;
        self.forget(relp);
        Ok(())
    }

    pub fn delete(&mut self, relp: &str) -> Result<()> {
        let p = resolve(&self.root, relp)?;
        std::fs::remove_file(&p)?;
        self.forget(relp);
        Ok(())
    }

    pub fn rename(&mut self, from: &str, to: &str) -> Result<()> {
        let a = resolve(&self.root, from)?;
        let b = resolve(&self.root, to)?;
        if b.exists() {
            bail!("{to} existe déjà");
        }
        if let Some(d) = b.parent() {
            std::fs::create_dir_all(d)?;
        }
        std::fs::rename(a, b)?;
        self.forget(from);
        Ok(())
    }

    pub fn read_all(&self) -> Vec<(String, String)> {
        let mut list = vec![];
        walk(&self.root, &self.root, &mut list, false);
        list.iter().filter_map(|p| std::fs::read_to_string(p).ok().map(|c| (rel(&self.root, p), c))).collect()
    }
}

#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn refuse_evasion() {
        let d = std::env::temp_dir().join("pop-link-test");
        std::fs::create_dir_all(&d).unwrap();
        assert!(resolve(&d, "../x.verse").is_err());
        assert!(resolve(&d, "/etc/x.verse").is_err());
        assert!(resolve(&d, "a/b.txt").is_err());
        assert!(resolve(&d, "Fortnite.digest.verse").is_err());
        assert!(resolve(&d, "Content/ok.verse").is_ok());
    }
}
