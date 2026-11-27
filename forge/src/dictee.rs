//! Dictée vocale locale (#356) : on garde le raccourci appuyé, Forge enregistre le micro, puis whisper.cpp
//! (licence MIT) transcrit sur le PC. Aucun son ne quitte le PC. Le modèle « large-v3-turbo q5_0 » (547 Mio) est
//! téléchargé au premier usage, seulement après l'accord de l'utilisateur, et vérifié par son empreinte SHA-1.
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::Serialize;
use sha1::{Digest, Sha1};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use tauri::{AppHandle, Manager};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

const FICHIER: &str = "ggml-large-v3-turbo-q5_0.bin";
const URL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin";
/// Taille et empreinte publiées par whisper.cpp (models/README.md).
pub const TAILLE: u64 = 574_041_195;
const SHA1: &str = "e050f7970618a659205450ad97eb95a18d69c9ee";
/// Au-delà, l'enregistrement s'arrête (une consigne, pas une réunion).
const SECONDES_MAX: usize = 120;

#[derive(Default)]
pub struct Dictee {
    recu: AtomicU64,
    en_cours: AtomicBool,
    enregistrement: Mutex<Option<Enregistrement>>,
    modele: Mutex<Option<Arc<WhisperContext>>>,
}

struct Enregistrement {
    arret: mpsc::Sender<()>,
    fil: std::thread::JoinHandle<Result<(Vec<f32>, u32), String>>,
    /// Texte provisoire (#477) : transcription répétée pendant qu'on parle.
    partiel: Arc<Mutex<(u64, String)>>,
    en_cours: Arc<AtomicBool>,
    transcripteur: Option<std::thread::JoinHandle<()>>,
}

fn chemin(app: &AppHandle) -> Result<PathBuf, String> {
    let d = app
        .path()
        .app_local_data_dir()
        .map_err(|e| e.to_string())?
        .join("modeles");
    Ok(d.join(FICHIER))
}

#[derive(Serialize)]
pub struct Etat {
    /// Le modèle est sur le PC.
    modele: bool,
    /// Taille à télécharger, en octets (annoncée avant tout téléchargement).
    taille: u64,
    /// Téléchargement en cours : octets reçus.
    recu: Option<u64>,
}

#[tauri::command]
pub fn dictee_etat(app: AppHandle, d: tauri::State<'_, Dictee>) -> Result<Etat, String> {
    let p = chemin(&app)?;
    Ok(Etat {
        modele: p.metadata().map(|m| m.len() == TAILLE).unwrap_or(false),
        taille: TAILLE,
        recu: d
            .en_cours
            .load(Ordering::Relaxed)
            .then(|| d.recu.load(Ordering::Relaxed)),
    })
}

/// Télécharge le modèle (appelé seulement après l'accord de l'utilisateur dans Forge). Vérifie taille et SHA-1
/// avant de le garder ; un fichier incomplet ou altéré est supprimé.
#[tauri::command]
pub async fn dictee_telecharger(app: AppHandle, d: tauri::State<'_, Dictee>) -> Result<(), String> {
    if d.en_cours.swap(true, Ordering::SeqCst) {
        return Err("Téléchargement déjà en cours.".into());
    }
    d.recu.store(0, Ordering::Relaxed);
    let r = telecharger(&app, &d).await;
    d.en_cours.store(false, Ordering::SeqCst);
    r
}

async fn telecharger(app: &AppHandle, d: &Dictee) -> Result<(), String> {
    let dest = chemin(app)?;
    std::fs::create_dir_all(dest.parent().unwrap()).map_err(|e| e.to_string())?;
    let part = dest.with_extension("part");
    let mut rep = reqwest::get(URL)
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("Téléchargement du modèle impossible : {e}"))?;
    let mut f = std::fs::File::create(&part).map_err(|e| e.to_string())?;
    let mut h = Sha1::new();
    let mut n = 0u64;
    while let Some(c) = rep
        .chunk()
        .await
        .map_err(|e| format!("Téléchargement interrompu : {e}"))?
    {
        use std::io::Write;
        f.write_all(&c).map_err(|e| e.to_string())?;
        h.update(&c);
        n += c.len() as u64;
        d.recu.store(n, Ordering::Relaxed);
    }
    drop(f);
    let empreinte: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if n != TAILLE || empreinte != SHA1 {
        let _ = std::fs::remove_file(&part);
        return Err("Le modèle téléchargé est incomplet ou altéré : réessayez.".into());
    }
    std::fs::rename(&part, &dest).map_err(|e| e.to_string())
}

/// Commence l'enregistrement du micro par défaut (raccourci appuyé).
// Asynchrone : charger le modèle la première fois prend quelques secondes, sans figer la fenêtre.
#[tauri::command]
pub async fn dictee_debut(
    app: AppHandle,
    d: tauri::State<'_, Dictee>,
    langue: Option<String>,
) -> Result<(), String> {
    let mut e = d.enregistrement.lock().unwrap();
    if e.is_some() {
        return Ok(());
    }
    let (arret, attente) = mpsc::channel::<()>();
    let (pret, ok) = mpsc::channel::<Result<(), String>>();
    let echantillons = Arc::new(Mutex::new(Vec::<f32>::new()));
    let taux_partage = Arc::new(AtomicU64::new(0));
    let (ech, tx) = (echantillons.clone(), taux_partage.clone());
    // Le flux du micro n'est pas transférable entre fils : il vit dans son propre fil jusqu'à l'arrêt.
    let fil = std::thread::spawn(move || {
        let echantillons = ech;
        let flux = (|| {
            let micro = cpal::default_host()
                .default_input_device()
                .ok_or("Aucun micro trouvé.")?;
            let cfg = micro.default_input_config().map_err(|e| e.to_string())?;
            let (taux, canaux) = (cfg.sample_rate().0, cfg.channels() as usize);
            let max = taux as usize * SECONDES_MAX;
            let e2 = echantillons.clone();
            let ajouter = move |mono: &mut dyn Iterator<Item = f32>| {
                let mut v = e2.lock().unwrap();
                if v.len() < max {
                    v.extend(mono);
                }
            };
            let err = |e| eprintln!("micro : {e}");
            let flux = match cfg.sample_format() {
                cpal::SampleFormat::F32 => micro.build_input_stream(
                    &cfg.into(),
                    move |x: &[f32], _: &_| {
                        ajouter(
                            &mut x
                                .chunks(canaux)
                                .map(|c| c.iter().sum::<f32>() / c.len() as f32),
                        )
                    },
                    err,
                    None,
                ),
                cpal::SampleFormat::I16 => micro.build_input_stream(
                    &cfg.into(),
                    move |x: &[i16], _: &_| {
                        ajouter(&mut x.chunks(canaux).map(|c| {
                            c.iter().map(|&s| s as f32 / 32768.0).sum::<f32>() / c.len() as f32
                        }))
                    },
                    err,
                    None,
                ),
                cpal::SampleFormat::U16 => micro.build_input_stream(
                    &cfg.into(),
                    move |x: &[u16], _: &_| {
                        ajouter(&mut x.chunks(canaux).map(|c| {
                            c.iter()
                                .map(|&s| (s as f32 - 32768.0) / 32768.0)
                                .sum::<f32>()
                                / c.len() as f32
                        }))
                    },
                    err,
                    None,
                ),
                f => return Err(format!("Format de micro non pris en charge : {f}")),
            }
            .map_err(|e| format!("Micro indisponible : {e}"))?;
            flux.play()
                .map_err(|e| format!("Micro indisponible : {e}"))?;
            Ok::<_, String>((flux, taux))
        })();
        match flux {
            Err(e) => {
                let _ = pret.send(Err(e.clone()));
                Err(e)
            }
            Ok((flux, taux)) => {
                tx.store(taux as u64, Ordering::SeqCst);
                let _ = pret.send(Ok(()));
                let _ = attente.recv();
                drop(flux);
                let v = std::mem::take(&mut *echantillons.lock().unwrap());
                Ok((v, taux))
            }
        }
    });
    ok.recv().map_err(|e| e.to_string())??;
    // Transcription en temps réel (#477) : tant qu'on parle, on retranscrit tout ce qui a été dit dès que la
    // transcription précédente est finie (les 30 dernières secondes au plus), et la page affiche ce texte provisoire.
    let partiel = Arc::new(Mutex::new((0u64, String::new())));
    let en_cours = Arc::new(AtomicBool::new(true));
    let modele = charger(&app, &d).ok();
    let transcripteur = modele.map(|m| {
        let (p, c, ech, tx) = (
            partiel.clone(),
            en_cours.clone(),
            echantillons.clone(),
            taux_partage.clone(),
        );
        let langue = langue_sure(langue);
        std::thread::spawn(move || {
            let mut deja = 0usize;
            while c.load(Ordering::SeqCst) {
                let taux = tx.load(Ordering::SeqCst) as usize;
                let son: Vec<f32> = {
                    let v = ech.lock().unwrap();
                    let debut = v.len().saturating_sub(taux * 30);
                    v[debut..].to_vec()
                };
                // Au moins 0,4 s de nouveau son avant de retranscrire.
                if taux == 0 || son.len() < taux / 2 || son.len() < deja + taux * 2 / 5 {
                    std::thread::sleep(std::time::Duration::from_millis(80));
                    continue;
                }
                deja = son.len();
                if let Ok(t) = transcrire(&m, &a_16k(&son, taux as u32), &langue) {
                    let mut g = p.lock().unwrap();
                    g.0 += 1;
                    g.1 = t;
                }
            }
        })
    });
    *e = Some(Enregistrement {
        arret,
        fil,
        partiel,
        en_cours,
        transcripteur,
    });
    Ok(())
}

fn langue_sure(langue: Option<String>) -> String {
    langue
        .filter(|l| l.len() == 2 && l.chars().all(|c| c.is_ascii_lowercase()))
        .unwrap_or_else(|| "fr".into())
}

/// Modèle chargé une seule fois (le premier chargement prend quelques secondes).
fn charger(app: &AppHandle, d: &Dictee) -> Result<Arc<WhisperContext>, String> {
    let mut m = d.modele.lock().unwrap();
    if let Some(c) = &*m {
        return Ok(c.clone());
    }
    let p = chemin(app)?;
    if !p.exists() {
        return Err("Le modèle de dictée n'est pas installé.".into());
    }
    let c = Arc::new(
        WhisperContext::new_with_params(&p, WhisperContextParameters::default())
            .map_err(|e| format!("Modèle de dictée illisible : {e}"))?,
    );
    *m = Some(c.clone());
    Ok(c)
}

#[derive(Serialize)]
pub struct Partiel {
    version: u64,
    texte: String,
}

/// Texte provisoire pendant qu'on parle (#477) : la page le relit et l'écrit dans le champ.
#[tauri::command]
pub fn dictee_partiel(d: tauri::State<'_, Dictee>) -> Option<Partiel> {
    let e = d.enregistrement.lock().unwrap();
    let g = e.as_ref()?.partiel.lock().unwrap().clone();
    Some(Partiel {
        version: g.0,
        texte: g.1,
    })
}

/// Raccourci relâché : arrête le micro et rend le texte transcrit (vide si rien n'a été dit).
#[tauri::command]
pub async fn dictee_fin(
    app: AppHandle,
    d: tauri::State<'_, Dictee>,
    langue: Option<String>,
) -> Result<String, String> {
    let e = d
        .enregistrement
        .lock()
        .unwrap()
        .take()
        .ok_or("Aucune dictée en cours.")?;
    let _ = e.arret.send(());
    e.en_cours.store(false, Ordering::SeqCst);
    let (son, taux) = e.fil.join().map_err(|_| "Erreur du micro.".to_string())??;
    if let Some(t) = e.transcripteur {
        let _ = tokio::task::spawn_blocking(move || t.join()).await;
    }
    if son.len() < taux as usize / 4 {
        return Ok(String::new());
    }
    let modele = charger(&app, &d)?;
    let langue = langue_sure(langue);
    tokio::task::spawn_blocking(move || transcrire(&modele, &a_16k(&son, taux), &langue))
        .await
        .map_err(|e| e.to_string())?
}

fn transcrire(modele: &WhisperContext, son: &[f32], langue: &str) -> Result<String, String> {
    let mut etat = modele.create_state().map_err(|e| e.to_string())?;
    let mut p = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    p.set_language(Some(langue));
    p.set_n_threads(
        std::thread::available_parallelism()
            .map(|n| n.get().min(8) as i32)
            .unwrap_or(4),
    );
    p.set_print_special(false);
    p.set_print_progress(false);
    p.set_print_realtime(false);
    p.set_print_timestamps(false);
    p.set_no_context(true);
    p.set_single_segment(false);
    etat.full(p, son)
        .map_err(|e| format!("Transcription impossible : {e}"))?;
    let mut texte = String::new();
    for s in etat.as_iter() {
        if let Ok(t) = s.to_str_lossy() {
            texte.push_str(&t);
        }
    }
    Ok(texte.trim().to_string())
}

/// Rééchantillonne en 16 kHz (format attendu par whisper), par interpolation linéaire.
fn a_16k(son: &[f32], taux: u32) -> Vec<f32> {
    if taux == 16_000 || son.is_empty() {
        return son.to_vec();
    }
    let ratio = taux as f64 / 16_000.0;
    let n = (son.len() as f64 / ratio) as usize;
    (0..n)
        .map(|i| {
            let x = i as f64 * ratio;
            let j = x as usize;
            let f = (x - j as f64) as f32;
            let a = son[j];
            let b = *son.get(j + 1).unwrap_or(&a);
            a + (b - a) * f
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reechantillonnage() {
        let s: Vec<f32> = (0..48_000).map(|i| (i as f32 / 48.0).sin()).collect();
        let r = a_16k(&s, 48_000);
        assert_eq!(r.len(), 16_000);
        assert!((r[100] - s[300]).abs() < 1e-6);
        assert_eq!(a_16k(&s[..10], 16_000).len(), 10);
    }
}
