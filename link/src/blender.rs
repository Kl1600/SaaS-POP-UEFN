//! Blender dans Forge (#351) : base des outils Texte 3D/2D (#324), Low Mesh (#325) et 3D Studio (#327).
//!
//! - Blender n'est ni inclus dans l'installateur ni modifié (licence GPL) : il est téléchargé au premier usage, après
//!   l'accord de l'utilisateur, depuis download.blender.org seulement, vérifié par l'empreinte SHA-256 publiée à côté,
//!   et décompressé dans le dossier de Forge. Version LTS fixée ci-dessous.
//! - Un script tourne isolé (Windows) : AppContainer sans aucun droit (ni réseau, ni fichiers de l'utilisateur), accès
//!   seulement à son dossier de travail temporaire ; Job Object avec mémoire maximale ; arrêt au temps maximum.
//! - Résultat : un fichier glTF (`.glb`), un aperçu PNG et le nombre de triangles.
use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

/// Version LTS fixée (support jusqu'en juillet 2027) ; une nouvelle version demande une mise à jour de Forge.
pub const VERSION: &str = "4.5.11";
const SERIE: &str = "4.5";
/// Temps maximum d'un script, par défaut.
pub const DELAI: Duration = Duration::from_secs(120);
/// Mémoire maximale de Blender (tous ses processus).
pub const MEMOIRE: u64 = 4 << 30;
/// Taille maximale d'un fichier produit rendu à POP.
const SORTIE_MAX: u64 = 50 << 20;

fn zip_nom() -> String {
    format!("blender-{VERSION}-windows-x64.zip")
}
pub fn url_zip() -> String {
    format!(
        "https://download.blender.org/release/Blender{SERIE}/{}",
        zip_nom()
    )
}
fn url_empreintes() -> String {
    format!("https://download.blender.org/release/Blender{SERIE}/blender-{VERSION}.sha256")
}

/// Dossier de Blender dans les données de Forge (%LOCALAPPDATA%\POP Forge\blender).
pub fn dossier() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(std::env::temp_dir);
    base.join("POP Forge").join("blender")
}

fn racine_version() -> PathBuf {
    dossier().join(VERSION)
}

/// `blender.exe` s'il est installé (dossier complet : la décompression a fini).
pub fn executable() -> Option<PathBuf> {
    let r = racine_version();
    if !r.join(".complet").exists() {
        return None;
    }
    let exe = r
        .join(format!("blender-{VERSION}-windows-x64"))
        .join("blender.exe");
    exe.exists().then_some(exe)
}

/// Téléchargement en cours (un seul à la fois) et octets reçus.
pub static EN_COURS: AtomicBool = AtomicBool::new(false);
pub static RECU: AtomicU64 = AtomicU64::new(0);
pub static TOTAL: AtomicU64 = AtomicU64::new(0);

#[derive(Serialize, Clone)]
pub struct Etat {
    pub installe: bool,
    pub version: &'static str,
    /// Taille du téléchargement en octets, annoncée avant tout téléchargement (0 si inconnue).
    pub taille: u64,
    /// Téléchargement en cours : octets reçus, puis `decompression`.
    pub recu: Option<u64>,
    pub decompression: bool,
    /// L'isolation n'existe que sous Windows : ailleurs, Blender n'est pas proposé.
    pub disponible: bool,
}

/// Taille annoncée : demandée au serveur de Blender (HEAD), sans rien télécharger.
pub async fn taille() -> u64 {
    let c = reqwest::Client::new();
    match c.head(url_zip()).send().await {
        Ok(r) if r.status().is_success() => r
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
        _ => 0,
    }
}

pub static DECOMPRESSION: AtomicBool = AtomicBool::new(false);

pub async fn etat() -> Etat {
    let installe = executable().is_some();
    let en_cours = EN_COURS.load(Ordering::Relaxed);
    let t = TOTAL.load(Ordering::Relaxed);
    Etat {
        installe,
        version: VERSION,
        taille: if installe {
            0
        } else if t > 0 {
            t
        } else {
            taille().await
        },
        recu: en_cours.then(|| RECU.load(Ordering::Relaxed)),
        decompression: en_cours && DECOMPRESSION.load(Ordering::Relaxed),
        disponible: cfg!(windows),
    }
}

/// Empreinte du zip dans le fichier publié par Blender (« <sha256>  <nom> » par ligne).
fn empreinte_publiee(fichier: &str, nom: &str) -> Option<String> {
    fichier.lines().find_map(|l| {
        let mut it = l.split_whitespace();
        let h = it.next()?;
        let n = it.next()?.trim_start_matches('*');
        (n == nom && h.len() == 64 && h.chars().all(|c| c.is_ascii_hexdigit()))
            .then(|| h.to_ascii_lowercase())
    })
}

/// Télécharge, vérifie et décompresse Blender. À n'appeler qu'après l'accord de l'utilisateur.
pub async fn installer() -> Result<()> {
    if !cfg!(windows) {
        bail!("Blender isolé n'est disponible que sous Windows.");
    }
    if EN_COURS.swap(true, Ordering::SeqCst) {
        bail!("Installation de Blender déjà en cours.");
    }
    RECU.store(0, Ordering::Relaxed);
    DECOMPRESSION.store(false, Ordering::Relaxed);
    let r = installer_vraiment().await;
    EN_COURS.store(false, Ordering::SeqCst);
    r
}

async fn installer_vraiment() -> Result<()> {
    let c = reqwest::Client::new();
    let publiees = c
        .get(url_empreintes())
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .context("Empreintes de Blender introuvables sur download.blender.org")?
        .text()
        .await?;
    let attendue = empreinte_publiee(&publiees, &zip_nom())
        .ok_or_else(|| anyhow!("Empreinte de {} absente du fichier publié.", zip_nom()))?;
    let d = dossier();
    std::fs::create_dir_all(&d)?;
    let part = d.join(format!("{}.part", zip_nom()));
    let mut rep = c
        .get(url_zip())
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .context("Téléchargement de Blender impossible")?;
    TOTAL.store(rep.content_length().unwrap_or(0), Ordering::Relaxed);
    let mut f = std::fs::File::create(&part)?;
    let mut h = Sha256::new();
    let mut n = 0u64;
    while let Some(b) = rep.chunk().await.context("Téléchargement interrompu")? {
        use std::io::Write;
        f.write_all(&b)?;
        h.update(&b);
        n += b.len() as u64;
        RECU.store(n, Ordering::Relaxed);
    }
    drop(f);
    if hex::encode(h.finalize()) != attendue {
        let _ = std::fs::remove_file(&part);
        bail!("Le fichier de Blender téléchargé est incomplet ou altéré : réessayez.");
    }
    DECOMPRESSION.store(true, Ordering::Relaxed);
    let racine = racine_version();
    let _ = std::fs::remove_dir_all(&racine);
    let (p, r) = (part.clone(), racine.clone());
    tokio::task::spawn_blocking(move || -> Result<()> {
        let mut z = zip::ZipArchive::new(std::fs::File::open(&p)?)?;
        z.extract(&r)
            .context("Décompression de Blender impossible")?;
        Ok(())
    })
    .await??;
    let _ = std::fs::remove_file(&part);
    #[cfg(windows)]
    isolation::autoriser_lecture(&racine)?;
    std::fs::write(racine.join(".complet"), VERSION)?;
    Ok(())
}

/// Résultat d'un script Blender.
#[derive(Serialize, Debug)]
pub struct Resultat {
    /// Fichier glTF binaire produit.
    #[serde(skip)]
    pub glb: Vec<u8>,
    /// Aperçu PNG.
    #[serde(skip)]
    pub apercu: Vec<u8>,
    pub triangles: u64,
    pub objets: u64,
    /// Autres fichiers produits par le script (`sortie.fbx`, `sortie.png`…), par nom.
    #[serde(skip)]
    pub autres: Vec<(String, Vec<u8>)>,
}

/// Script ajouté autour de celui de l'IA ou de l'outil : journal dans le dossier de travail, export glTF si le script
/// n'a rien exporté, nombre de triangles et aperçu PNG (Cycles sur le processeur : pas besoin de carte graphique).
pub const ENVELOPPE: &str = r#"
import bpy, os, sys, json, math, traceback
from mathutils import Vector
T = os.path.dirname(os.path.abspath(__file__))
sys.stdout = sys.stderr = open(os.path.join(T, "journal.txt"), "w", encoding="utf-8", buffering=1)
def fin(msg):
    open(os.path.join(T, "erreur.txt"), "w", encoding="utf-8").write(msg)
    sys.exit(1)
# Scène vide : sans le cube, la caméra et la lampe de la scène par défaut.
bpy.ops.wm.read_factory_settings(use_empty=True)
try:
    src = open(os.path.join(T, "script.py"), encoding="utf-8").read()
    exec(compile(src, "script.py", "exec"), {"__name__": "__main__", "TRAVAIL": T})
except SystemExit:
    raise
except BaseException:
    fin(traceback.format_exc(limit=4))
sc = bpy.context.scene
objets = [o for o in sc.objects if o.type == "MESH"]
if not objets:
    fin("Le script n'a créé aucun objet 3D.")
dg = bpy.context.evaluated_depsgraph_get()
tri = 0
pts = []
for o in objets:
    e = o.evaluated_get(dg)
    m = e.to_mesh()
    m.calc_loop_triangles()
    tri += len(m.loop_triangles)
    pts += [e.matrix_world @ v.co for v in m.vertices]
    e.to_mesh_clear()
sortie = os.path.join(T, "sortie.glb")
if not os.path.exists(sortie):
    bpy.ops.export_scene.gltf(filepath=sortie, export_format="GLB")
lo = Vector((min(p.x for p in pts), min(p.y for p in pts), min(p.z for p in pts)))
hi = Vector((max(p.x for p in pts), max(p.y for p in pts), max(p.z for p in pts)))
c = (lo + hi) / 2
r = max((hi - lo).length / 2, 0.01)
cam = bpy.data.objects.new("pop_camera", bpy.data.cameras.new("pop_camera"))
sc.collection.objects.link(cam)
d = Vector((1.0, -1.2, 0.8)).normalized()
cam.location = c + d * r / math.sin(math.radians(20)) * 1.15
cam.rotation_euler = (-d).to_track_quat("-Z", "Y").to_euler()
cam.data.angle = math.radians(40)
cam.data.clip_end = r * 100
sc.camera = cam
sol = bpy.data.objects.new("pop_soleil", bpy.data.lights.new("pop_soleil", "SUN"))
sol.data.energy = 3
sol.rotation_euler = (math.radians(50), 0, math.radians(30))
sc.collection.objects.link(sol)
if sc.world is None:
    sc.world = bpy.data.worlds.new("pop_monde")
sc.world.color = (0.35, 0.35, 0.35)
sc.render.engine = "CYCLES"
sc.cycles.device = "CPU"
sc.cycles.samples = 16
sc.render.resolution_x = sc.render.resolution_y = 512
sc.render.resolution_percentage = 100
sc.render.film_transparent = True
sc.render.image_settings.file_format = "PNG"
sc.render.filepath = os.path.join(T, "apercu.png")
bpy.ops.render.render(write_still=True)
json.dump({"triangles": tri, "objets": len(objets)}, open(os.path.join(T, "infos.json"), "w"))
"#;

/// Message clair pour une erreur du script (accès refusé par l'isolation, réseau…).
pub fn expliquer(erreur: &str) -> String {
    let e = erreur.to_lowercase();
    let dernier = erreur.trim().lines().last().unwrap_or("").trim();
    if e.contains("10013")
        || e.contains("socket")
        || e.contains("urlopen")
        || e.contains("getaddrinfo")
        || e.contains("11001")
    {
        format!("Accès réseau interdit : un script Blender lancé par Forge ne peut pas se connecter à Internet. ({dernier})")
    } else if e.contains("permissionerror")
        || e.contains("access is denied")
        || e.contains("accès refusé")
        || e.contains("winerror 5")
    {
        format!("Accès refusé : un script Blender lancé par Forge ne peut lire et écrire que dans son dossier de travail. ({dernier})")
    } else {
        format!("Le script Blender a échoué : {}", erreur.trim())
    }
}

/// Lance un script Blender isolé, avec des fichiers d'entrée copiés dans son dossier de travail.
pub async fn lancer(
    script: &str,
    entrees: Vec<(String, Vec<u8>)>,
    delai: Duration,
) -> Result<Resultat> {
    let exe = executable().ok_or_else(|| {
        anyhow!("Blender n'est pas installé : Forge → Outils → Blender, « Installer ».")
    })?;
    let travail =
        dossier()
            .join("travail")
            .join(format!("{}-{}", std::process::id(), rand::random::<u32>()));
    std::fs::create_dir_all(&travail)?;
    let r = lancer_dans(&exe, &travail, script, entrees, delai).await;
    let _ = std::fs::remove_dir_all(&travail);
    r
}

async fn lancer_dans(
    exe: &Path,
    travail: &Path,
    script: &str,
    entrees: Vec<(String, Vec<u8>)>,
    delai: Duration,
) -> Result<Resultat> {
    for (nom, data) in entrees {
        // Nom simple seulement : pas de chemin.
        let nom: String = nom
            .chars()
            .filter(|c| c.is_alphanumeric() || "._- ".contains(*c))
            .collect();
        if nom.is_empty() || nom.starts_with('.') || nom == "script.py" || nom == "pop_lancer.py" {
            bail!("Nom de fichier d'entrée refusé.");
        }
        std::fs::write(travail.join(nom), data)?;
    }
    std::fs::write(travail.join("script.py"), script)?;
    std::fs::write(travail.join("pop_lancer.py"), ENVELOPPE)?;
    std::fs::create_dir_all(travail.join("utilisateur"))?;
    let args: Vec<String> = vec![
        "--background".into(),
        "--factory-startup".into(),
        "--python-exit-code".into(),
        "1".into(),
        "--python".into(),
        travail.join("pop_lancer.py").to_string_lossy().into_owned(),
    ];
    let (e, t) = (exe.to_path_buf(), travail.to_path_buf());
    let fin = tokio::task::spawn_blocking(move || executer(&e, &args, &t, delai)).await??;
    let lire = |n: &str| std::fs::read_to_string(travail.join(n)).unwrap_or_default();
    match fin {
        Fin::Delai => bail!(
            "Le script Blender a dépassé {} s : il a été arrêté.",
            delai.as_secs()
        ),
        Fin::Code(0) => {}
        Fin::Code(_) => {
            let err = lire("erreur.txt");
            let journal = lire("journal.txt");
            let texte = if err.trim().is_empty() {
                journal
                    .lines()
                    .rev()
                    .take(6)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                err
            };
            if texte.trim().is_empty() {
                bail!("Blender s'est arrêté sans message (mémoire maximale atteinte, ou isolation refusée par Windows).");
            }
            bail!("{}", expliquer(&texte));
        }
    }
    let glb = travail.join("sortie.glb");
    let m = std::fs::metadata(&glb).context("Blender n'a produit aucun fichier glTF.")?;
    if m.len() > SORTIE_MAX {
        bail!(
            "Le modèle produit est trop gros ({} Mo, 50 Mo au plus).",
            m.len() >> 20
        );
    }
    let infos: serde_json::Value = serde_json::from_str(&lire("infos.json")).unwrap_or_default();
    let mut autres = vec![];
    for e in std::fs::read_dir(travail)?.flatten() {
        let nom = e.file_name().to_string_lossy().into_owned();
        if nom.starts_with("sortie.")
            && nom != "sortie.glb"
            && e.metadata().map(|m| m.len() <= SORTIE_MAX).unwrap_or(false)
        {
            autres.push((nom, std::fs::read(e.path())?));
        }
    }
    Ok(Resultat {
        autres,
        glb: std::fs::read(&glb)?,
        apercu: std::fs::read(travail.join("apercu.png")).unwrap_or_default(),
        triangles: infos["triangles"].as_u64().unwrap_or(0),
        objets: infos["objets"].as_u64().unwrap_or(0),
    })
}

#[derive(Debug, PartialEq)]
#[cfg_attr(not(windows), allow(dead_code))]
enum Fin {
    Code(u32),
    Delai,
}

#[cfg(not(windows))]
fn executer(_exe: &Path, _args: &[String], _travail: &Path, _delai: Duration) -> Result<Fin> {
    bail!("Blender isolé n'est disponible que sous Windows.")
}

#[cfg(windows)]
fn executer(exe: &Path, args: &[String], travail: &Path, delai: Duration) -> Result<Fin> {
    isolation::autoriser_travail(travail)?;
    isolation::lancer(exe, args, travail, delai)
}

/// Isolation Windows : AppContainer sans capacité (pas de réseau, pas de fichiers de l'utilisateur) et Job Object.
#[cfg(windows)]
mod isolation {
    use super::Fin;
    use anyhow::{bail, Result};
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::process::CommandExt;
    use std::path::Path;
    use std::time::Duration;
    use windows_sys::Win32::Foundation::*;
    use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
    use windows_sys::Win32::Security::Isolation::*;
    use windows_sys::Win32::Security::*;
    use windows_sys::Win32::System::JobObjects::*;
    use windows_sys::Win32::System::Threading::*;

    const NOM: &str = "POP.Forge.Blender";

    fn large(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    /// SID de l'AppContainer de Forge (créé au premier usage).
    struct Sid(PSID);
    impl Drop for Sid {
        fn drop(&mut self) {
            unsafe { FreeSid(self.0) };
        }
    }

    fn sid() -> Result<Sid> {
        let nom = large(NOM);
        let affiche = large("POP Forge · Blender");
        let mut s: PSID = std::ptr::null_mut();
        let hr = unsafe {
            CreateAppContainerProfile(
                nom.as_ptr(),
                affiche.as_ptr(),
                affiche.as_ptr(),
                std::ptr::null(),
                0,
                &mut s,
            )
        };
        if hr < 0 {
            let hr2 = unsafe { DeriveAppContainerSidFromAppContainerName(nom.as_ptr(), &mut s) };
            if hr2 < 0 {
                bail!("Isolation de Blender impossible (AppContainer, code {hr:#x}).");
            }
        }
        Ok(Sid(s))
    }

    fn sid_texte(s: &Sid) -> Result<String> {
        let mut p: *mut u16 = std::ptr::null_mut();
        if unsafe { ConvertSidToStringSidW(s.0, &mut p) } == 0 {
            bail!("Isolation de Blender impossible (SID).");
        }
        let mut n = 0;
        while unsafe { *p.add(n) } != 0 {
            n += 1;
        }
        let t = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(p, n) });
        unsafe { LocalFree(p as _) };
        Ok(t)
    }

    /// Droits du conteneur sur un dossier (et tout ce qu'il contient), par icacls.
    fn accorder(dossier: &Path, droits: &str) -> Result<()> {
        let s = sid_texte(&sid()?)?;
        let o = std::process::Command::new("icacls")
            .arg(dossier)
            .arg("/grant")
            .arg(format!("*{s}:(OI)(CI){droits}"))
            .arg("/T")
            .arg("/Q")
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
            .output()?;
        if !o.status.success() {
            bail!(
                "Isolation de Blender impossible (droits sur {}) : {}",
                dossier.display(),
                String::from_utf8_lossy(&o.stderr).trim()
            );
        }
        Ok(())
    }

    /// Lecture et exécution de Blender par le conteneur (une fois, après la décompression).
    pub fn autoriser_lecture(racine: &Path) -> Result<()> {
        accorder(racine, "(RX)")
    }

    /// Lecture et écriture du dossier de travail, et de lui seul.
    pub fn autoriser_travail(travail: &Path) -> Result<()> {
        accorder(travail, "(M)")
    }

    struct H(HANDLE);
    impl Drop for H {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { CloseHandle(self.0) };
            }
        }
    }

    pub fn lancer(exe: &Path, args: &[String], travail: &Path, delai: Duration) -> Result<Fin> {
        let s = sid()?;
        let mut caps = SECURITY_CAPABILITIES {
            AppContainerSid: s.0,
            Capabilities: std::ptr::null_mut(),
            CapabilityCount: 0,
            Reserved: 0,
        };
        // Liste d'attributs : les capacités de sécurité (aucune) de l'AppContainer.
        let mut taille: usize = 0;
        unsafe { InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut taille) };
        let mut tampon = vec![0u8; taille];
        let liste = tampon.as_mut_ptr() as LPPROC_THREAD_ATTRIBUTE_LIST;
        if unsafe { InitializeProcThreadAttributeList(liste, 1, 0, &mut taille) } == 0 {
            bail!("Isolation de Blender impossible (attributs).");
        }
        let ok = unsafe {
            UpdateProcThreadAttribute(
                liste,
                0,
                PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize,
                &mut caps as *mut _ as *const _,
                std::mem::size_of::<SECURITY_CAPABILITIES>(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        };
        if ok == 0 {
            unsafe { DeleteProcThreadAttributeList(liste) };
            bail!("Isolation de Blender impossible (capacités).");
        }
        let mut si: STARTUPINFOEXW = unsafe { std::mem::zeroed() };
        si.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
        si.lpAttributeList = liste;
        // Ligne de commande : chaque argument entre guillemets.
        let mut ligne = format!("\"{}\"", exe.display());
        for a in args {
            ligne.push_str(&format!(" \"{}\"", a.replace('"', "")));
        }
        let mut ligne = large(&ligne);
        // Environnement réduit : dossiers temporaires et préférences de Blender dans le dossier de travail.
        let t = travail.display().to_string();
        let mut env = String::new();
        for (k, v) in [
            (
                "SystemRoot",
                std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into()),
            ),
            ("TEMP", t.clone()),
            ("TMP", t.clone()),
            ("BLENDER_USER_RESOURCES", format!("{t}\\utilisateur")),
            ("BLENDER_USER_CONFIG", format!("{t}\\utilisateur\\config")),
            ("BLENDER_USER_SCRIPTS", format!("{t}\\utilisateur\\scripts")),
            ("PYTHONNOUSERSITE", "1".into()),
            (
                "NUMBER_OF_PROCESSORS",
                std::env::var("NUMBER_OF_PROCESSORS").unwrap_or_else(|_| "4".into()),
            ),
        ] {
            env.push_str(&format!("{k}={v}\0"));
        }
        env.push('\0');
        let env: Vec<u16> = env.encode_utf16().collect();
        let dossier: Vec<u16> = travail.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut pi: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
        let ok = unsafe {
            CreateProcessW(
                std::ptr::null(),
                ligne.as_mut_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                0,
                EXTENDED_STARTUPINFO_PRESENT
                    | CREATE_SUSPENDED
                    | CREATE_UNICODE_ENVIRONMENT
                    | 0x0800_0000,
                env.as_ptr() as *const _,
                dossier.as_ptr(),
                &si.StartupInfo,
                &mut pi,
            )
        };
        unsafe { DeleteProcThreadAttributeList(liste) };
        if ok == 0 {
            let e = std::io::Error::last_os_error();
            bail!("Blender n'a pas pu démarrer dans son isolation : {e}");
        }
        let (processus, fil) = (H(pi.hProcess), H(pi.hThread));
        // Job Object : mémoire maximale, et tout s'arrête si Forge ferme le job.
        let job = H(unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) });
        if job.0.is_null() {
            unsafe { TerminateProcess(processus.0, 1) };
            bail!("Isolation de Blender impossible (job).");
        }
        let mut lim: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        lim.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_JOB_MEMORY
            | JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
            | JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION;
        lim.JobMemoryLimit = super::MEMOIRE as usize;
        let ok = unsafe {
            SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                &lim as *const _ as *const _,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        } != 0
            && unsafe { AssignProcessToJobObject(job.0, processus.0) } != 0;
        if !ok {
            unsafe { TerminateProcess(processus.0, 1) };
            bail!("Isolation de Blender impossible (limites).");
        }
        unsafe { ResumeThread(fil.0) };
        let ms = delai.as_millis().min(u32::MAX as u128 - 1) as u32;
        if unsafe { WaitForSingleObject(processus.0, ms) } == WAIT_TIMEOUT {
            unsafe { TerminateJobObject(job.0, 1) };
            unsafe { WaitForSingleObject(processus.0, 5000) };
            return Ok(Fin::Delai);
        }
        let mut code = 0u32;
        unsafe { GetExitCodeProcess(processus.0, &mut code) };
        Ok(Fin::Code(code))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empreinte_du_fichier_publie() {
        let f = "aa  blender-4.5.11-linux-x64.tar.xz\n\
                 0123456789abcdef0123456789abcdef0123456789ABCDEF0123456789abcdef  blender-4.5.11-windows-x64.zip\n";
        assert_eq!(
            empreinte_publiee(f, &zip_nom()).as_deref(),
            Some("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
        );
        assert_eq!(empreinte_publiee(f, "autre.zip"), None);
        assert!(url_zip().starts_with("https://download.blender.org/release/Blender4.5/"));
    }

    #[test]
    fn messages_clairs() {
        assert!(expliquer(
            "Traceback…\nPermissionError: [WinError 10013] An attempt was made to access a socket"
        )
        .starts_with("Accès réseau interdit"));
        assert!(expliquer(
            "urllib.error.URLError: <urlopen error [Errno 11001] getaddrinfo failed>"
        )
        .starts_with("Accès réseau interdit"));
        assert!(expliquer("PermissionError: [Errno 13] Permission denied: 'C:\\\\Users\\\\x\\\\Documents\\\\a.txt'\nPermissionError").starts_with("Accès refusé"));
        assert!(expliquer("NameError: name 'x' is not defined")
            .starts_with("Le script Blender a échoué"));
    }

    #[tokio::test]
    async fn sans_blender_message_clair() {
        if executable().is_none() {
            let e = lancer("import bpy", vec![], DELAI)
                .await
                .unwrap_err()
                .to_string();
            assert!(e.contains("Installer"), "{e}");
        }
    }
}
