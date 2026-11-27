//! Miniatures (#307) : bibliothèque interne de POP (inspirations et ingrédients, gérée par l'équipe POP dans
//! l'Administration) et composition d'une miniature conforme aux règles d'Epic à partir des vraies photos de l'île
//! (#306). POP n'appelle aucune IA : l'IA du client choisit les photos, le titre et le style ; POP dessine l'image et
//! la contrôle avant de la rendre (taille, mots interdits, commandes d'une plateforme, ressemblance à une inspiration).
use crate::{
    auth::{random_token, AuthUser},
    error::{bad, ApiResult, AppError},
    s3::S3,
    AppState,
};
use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use axum::{
    extract::{Path, State},
    Json,
};
use image::{imageops::FilterType, DynamicImage, GenericImageView, Rgba, RgbaImage};
use serde::Deserialize;
use serde_json::{json, Value};

/// Police par défaut, libre (SIL Open Font License, `assets/polices/Poppins-OFL.txt`).
const POLICE: &[u8] = include_bytes!("../assets/polices/Poppins-Bold.ttf");
pub const LARGEUR: u32 = 1920;
pub const HAUTEUR: u32 = 1080;
pub const LOGO_L: u32 = 1440;
pub const LOGO_H: u32 = 608;
/// Taille maximale d'une miniature acceptée par Epic.
pub const MAX_OCTETS: usize = 5 * 1024 * 1024;
/// En dessous de cette distance (sur 64 bits d'empreinte), l'image est trop proche d'une inspiration.
pub const TROP_PROCHE: u32 = 10;
const MAX_FICHIER: usize = 10 * 1024 * 1024;

/// Mots refusés par Epic dans une miniature (récompenses trompeuses, marques d'Epic et d'autres jeux).
const INTERDITS: &[&str] = &[
    "xp",
    "afk",
    "farm",
    "farming",
    "piece",
    "pieces",
    "coin",
    "coins",
    "vbucks",
    "v-bucks",
    "vbuck",
    "robux",
    "fortnite",
    "epic",
    "gratuit",
    "gratuits",
    "free",
    "battlepass",
    "level up",
    "passe de combat",
    "v bucks",
];
/// Commandes d'une plateforme (une miniature ne montre pas de touche ni de manette).
const COMMANDES: &[&str] = &[
    "ps4",
    "ps5",
    "playstation",
    "xbox",
    "nintendo",
    "manette",
    "clavier",
    "souris",
    "appuie",
    "appuyez",
    "press",
    "clique",
    "cliquez",
    "click",
    "touche",
];

fn sans_accents(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'à' | 'â' | 'ä' | 'á' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            c => c,
        })
        .collect()
}

/// Contrôle du texte d'une miniature : rend la raison du refus, que l'IA corrige elle-même.
pub fn verifier_texte(texte: &str) -> Result<(), String> {
    let t = sans_accents(texte);
    let mots: Vec<&str> = t
        .split(|c: char| !c.is_alphanumeric() && c != '-')
        .filter(|m| !m.is_empty())
        .collect();
    let colle = t.replace([' ', '-', '_', '.'], "");
    let trouve = |liste: &[&str]| -> Vec<String> {
        liste
            .iter()
            .filter(|m| {
                if m.contains(' ') {
                    t.contains(*m)
                } else {
                    mots.iter().any(|x| x == *m || x.replace('-', "") == **m)
                        || (m.len() >= 5 && colle.contains(&m.replace('-', "")))
                }
            })
            .map(|m| m.replace(['-', ' '], ""))
            .collect::<Vec<String>>()
    };
    // Une seule fois par mot (« V-BUCKS », « VBUCKS » et « VBUCK » → « VBUCKS »).
    let propres = |l: Vec<String>| -> Vec<String> {
        let mut v: Vec<String> = vec![];
        for m in &l {
            if !l.iter().any(|x| x != m && x.contains(m.as_str())) && !v.contains(m) {
                v.push(m.clone());
            }
        }
        v.into_iter()
            .map(|m| format!("« {} »", m.to_uppercase()))
            .collect()
    };
    let i = propres(trouve(INTERDITS));
    if !i.is_empty() {
        return Err(format!(
            "Mots interdits par Epic dans une miniature : {}. Retirez-les (pas de récompense promise, pas de nom d'Epic, de Fortnite ou d'un autre jeu).",
            i.join(", ")
        ));
    }
    let c = propres(trouve(COMMANDES));
    if !c.is_empty() {
        return Err(format!(
            "Pas de commandes d'une plateforme dans une miniature : {}. Retirez-les.",
            c.join(", ")
        ));
    }
    Ok(())
}

/// Empreinte de différence (dHash 64 bits) : deux images proches ont peu de bits différents.
pub fn empreinte(img: &DynamicImage) -> u64 {
    let g = img.resize_exact(9, 8, FilterType::Triangle).to_luma8();
    let mut h = 0u64;
    for y in 0..8 {
        for x in 0..8 {
            h = (h << 1) | (g.get_pixel(x, y)[0] < g.get_pixel(x + 1, y)[0]) as u64;
        }
    }
    h
}

pub fn distance(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

/// L'image ressemble-t-elle trop à une inspiration ? Rend le titre de la plus proche.
pub fn trop_proche(e: u64, inspirations: &[(String, u64)]) -> Option<String> {
    inspirations
        .iter()
        .filter(|(_, h)| distance(e, *h) <= TROP_PROCHE)
        .min_by_key(|(_, h)| distance(e, *h))
        .map(|(t, _)| t.clone())
}

fn couleur(s: Option<&str>, defaut: [u8; 4]) -> Result<Rgba<u8>, String> {
    let Some(s) = s.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(Rgba(defaut));
    };
    let h = s.trim_start_matches('#');
    if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!(
            "Couleur invalide « {s} » : écrivez-la #RRVVBB (ex. #FFD400)."
        ));
    }
    let v = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap();
    Ok(Rgba([v(0), v(2), v(4), 255]))
}

fn melanger(img: &mut RgbaImage, x: i32, y: i32, c: Rgba<u8>, a: f32) {
    if x < 0 || y < 0 || x >= img.width() as i32 || y >= img.height() as i32 || a <= 0.0 {
        return;
    }
    let p = img.get_pixel_mut(x as u32, y as u32);
    let a = (a * c[3] as f32 / 255.0).clamp(0.0, 1.0);
    let ad = p[3] as f32 / 255.0;
    let ao = a + ad * (1.0 - a);
    for i in 0..3 {
        let v = if ao > 0.0 {
            (c[i] as f32 * a + p[i] as f32 * ad * (1.0 - a)) / ao
        } else {
            0.0
        };
        p[i] = v.round() as u8;
    }
    p[3] = (ao * 255.0).round() as u8;
}

/// Largeur d'un texte à une taille donnée.
fn largeur(police: &FontRef, taille: f32, texte: &str) -> f32 {
    let f = police.as_scaled(PxScale::from(taille));
    let mut l = 0.0;
    let mut avant = None;
    for c in texte.chars() {
        let g = f.glyph_id(c);
        if let Some(a) = avant {
            l += f.kern(a, g);
        }
        l += f.h_advance(g);
        avant = Some(g);
    }
    l
}

/// Plus grande taille (jusqu'à `max`) qui fait tenir le texte dans `boite` de large.
fn taille_pour(police: &FontRef, texte: &str, boite: f32, max: f32) -> f32 {
    let l = largeur(police, 100.0, texte).max(1.0);
    (100.0 * boite / l).min(max).max(12.0)
}

/// Écrit un texte centré sur `cx`, ligne de base `y`, avec un contour épais (lisible sur une photo).
fn ecrire(
    img: &mut RgbaImage,
    police: &FontRef,
    taille: f32,
    texte: &str,
    cx: f32,
    y: f32,
    plein: Rgba<u8>,
    contour: Rgba<u8>,
) {
    let f = police.as_scaled(PxScale::from(taille));
    let mut x = cx - largeur(police, taille, texte) / 2.0;
    let mut glyphes = vec![];
    let mut avant = None;
    for c in texte.chars() {
        let id = f.glyph_id(c);
        if let Some(a) = avant {
            x += f.kern(a, id);
        }
        glyphes.push(id.with_scale_and_position(PxScale::from(taille), ab_glyph::point(x, y)));
        x += f.h_advance(id);
        avant = Some(id);
    }
    let r = (taille / 14.0).max(2.0) as i32;
    for (passe, c) in [(0, contour), (1, plein)] {
        for g in &glyphes {
            let Some(o) = police.outline_glyph(g.clone()) else {
                continue;
            };
            let b = o.px_bounds();
            let decalages: Vec<(i32, i32)> = if passe == 0 {
                (-r..=r)
                    .flat_map(|dx| (-r..=r).map(move |dy| (dx, dy)))
                    .filter(|(dx, dy)| dx * dx + dy * dy <= r * r)
                    .collect()
            } else {
                vec![(0, 0)]
            };
            o.draw(|gx, gy, a| {
                for (dx, dy) in &decalages {
                    melanger(
                        img,
                        b.min.x as i32 + gx as i32 + dx,
                        b.min.y as i32 + gy as i32 + dy,
                        c,
                        a,
                    );
                }
            });
        }
    }
}

/// Ce que l'IA du client demande de dessiner.
pub struct Demande<'a> {
    pub titre: &'a str,
    pub sous_titre: Option<&'a str>,
    pub couleur: Option<&'a str>,
    pub contour: Option<&'a str>,
    /// haut, centre ou bas
    pub position: Option<&'a str>,
}

/// Dessine la miniature 1920 × 1080 et le logo 1440 × 608 (fond transparent). `fond` : la photo de l'île (ou le fond
/// fourni par l'IA du client) ; `calques` : cadres et effets de la bibliothèque, posés par-dessus.
pub fn composer(
    fond: &DynamicImage,
    calques: &[DynamicImage],
    d: &Demande,
    police: &FontRef,
) -> Result<(RgbaImage, RgbaImage), String> {
    let plein = couleur(d.couleur, [255, 212, 0, 255])?;
    let contour = couleur(d.contour, [12, 12, 12, 255])?;
    let titre = d.titre.trim();
    if titre.is_empty() || titre.chars().count() > 40 {
        return Err("Le titre de la miniature doit faire de 1 à 40 caractères.".into());
    }
    let mut img = fond
        .resize_to_fill(LARGEUR, HAUTEUR, FilterType::Lanczos3)
        .to_rgba8();
    for c in calques {
        let c = c
            .resize_to_fill(LARGEUR, HAUTEUR, FilterType::Triangle)
            .to_rgba8();
        for (x, y, p) in c.enumerate_pixels() {
            melanger(
                &mut img,
                x as i32,
                y as i32,
                Rgba([p[0], p[1], p[2], 255]),
                p[3] as f32 / 255.0,
            );
        }
    }
    let haut = match d.position.unwrap_or("haut") {
        "haut" => 0.2,
        "centre" => 0.55,
        "bas" => 0.86,
        p => return Err(format!("Position « {p} » inconnue : haut, centre ou bas.")),
    };
    let t = titre.to_uppercase();
    let taille = taille_pour(police, &t, LARGEUR as f32 * 0.86, 230.0);
    let sous = d.sous_titre.map(str::trim).filter(|s| !s.is_empty());
    if sous.is_some_and(|s| s.chars().count() > 60) {
        return Err("Le sous-titre doit faire 60 caractères au plus.".into());
    }
    let ts = sous
        .map(|s| taille_pour(police, s, LARGEUR as f32 * 0.7, 90.0))
        .unwrap_or(0.0);
    // Le bloc titre + sous-titre reste dans l'image, à 60 px des bords au moins.
    let mut cy = HAUTEUR as f32 * haut;
    let bas = cy + taille * 0.35 + ts * 1.3 + ts * 0.3;
    if bas > HAUTEUR as f32 - 60.0 {
        cy -= bas - (HAUTEUR as f32 - 60.0);
    }
    if cy - taille * 0.75 < 60.0 {
        cy = 60.0 + taille * 0.75;
    }
    // Bande assombrie sous le texte, pour qu'il reste lisible sur n'importe quelle photo.
    let centre = cy + ts * 0.6;
    for y in 0..HAUTEUR {
        let e = ((y as f32 - centre).abs() / 260.0).min(1.0);
        let a = 0.45 * (1.0 - e * e);
        if a > 0.0 {
            for x in 0..LARGEUR {
                melanger(&mut img, x as i32, y as i32, Rgba([0, 0, 0, 255]), a);
            }
        }
    }
    ecrire(
        &mut img,
        police,
        taille,
        &t,
        LARGEUR as f32 / 2.0,
        cy + taille * 0.35,
        plein,
        contour,
    );
    if let Some(s) = sous {
        ecrire(
            &mut img,
            police,
            ts,
            s,
            LARGEUR as f32 / 2.0,
            cy + taille * 0.35 + ts * 1.3,
            Rgba([255, 255, 255, 255]),
            contour,
        );
    }
    let mut logo = RgbaImage::from_pixel(LOGO_L, LOGO_H, Rgba([0, 0, 0, 0]));
    let tl = taille_pour(police, &t, LOGO_L as f32 * 0.9, 360.0);
    ecrire(
        &mut logo,
        police,
        tl,
        &t,
        LOGO_L as f32 / 2.0,
        LOGO_H as f32 / 2.0 + tl * 0.35,
        plein,
        contour,
    );
    Ok((img, logo))
}

/// Encode la miniature : PNG s'il tient dans 5 Mo, sinon JPEG ; le logo toujours en PNG (transparent).
pub fn encoder(
    img: &RgbaImage,
    logo: &RgbaImage,
) -> Result<((Vec<u8>, &'static str), Vec<u8>), String> {
    let png = |i: &RgbaImage| {
        let mut v = std::io::Cursor::new(vec![]);
        i.write_to(&mut v, image::ImageFormat::Png)
            .map(|_| v.into_inner())
            .map_err(|e| e.to_string())
    };
    let mut m = (png(img)?, "png");
    for q in [92u8, 85, 75] {
        if m.0.len() <= MAX_OCTETS {
            break;
        }
        let mut v = vec![];
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut v, q)
            .encode_image(&DynamicImage::ImageRgba8(img.clone()).to_rgb8())
            .map_err(|e| e.to_string())?;
        m = (v, "jpg");
    }
    if m.0.len() > MAX_OCTETS {
        return Err("La miniature dépasse 5 Mo.".into());
    }
    Ok((m, png(logo)?))
}

/// Contrôle final : dimensions, poids, transparence du logo.
pub fn controler(img: &DynamicImage, octets: usize, logo: &DynamicImage) -> Result<(), String> {
    if img.dimensions() != (LARGEUR, HAUTEUR) {
        return Err(format!("La miniature doit faire {LARGEUR} × {HAUTEUR}."));
    }
    if octets > MAX_OCTETS {
        return Err("La miniature dépasse 5 Mo.".into());
    }
    if logo.dimensions() != (LOGO_L, LOGO_H)
        || !logo.color().has_alpha()
        || logo.get_pixel(0, 0)[3] != 0
    {
        return Err(format!(
            "Le logo doit être un PNG transparent de {LOGO_L} × {LOGO_H}."
        ));
    }
    Ok(())
}

// ---------- Bibliothèque (Administration, équipe POP seulement) ----------

const TIROIRS: &[&str] = &["inspiration", "ingredient"];
const SORTES: &[&str] = &["image", "police", "cadre", "effet", "fond"];

#[derive(Deserialize)]
pub struct ElementIn {
    drawer: String,
    kind: String,
    title: String,
    #[serde(default)]
    keywords: String,
    #[serde(default)]
    source: String,
    #[serde(default)]
    license: String,
    /// Fichier en base64 (10 Mo au plus) ; sinon `url` d'une image (inspiration).
    #[serde(default)]
    data: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

/// Règles d'un élément avant tout envoi : tiroir, sorte, licence obligatoire pour un ingrédient.
pub fn valider(e: &ElementIn) -> Result<(), AppError> {
    if !TIROIRS.contains(&e.drawer.as_str()) {
        return Err(bad("Tiroir : inspiration ou ingredient."));
    }
    if !SORTES.contains(&e.kind.as_str()) {
        return Err(bad("Sorte : image, police, cadre, effet ou fond."));
    }
    if e.drawer == "inspiration" && e.kind != "image" {
        return Err(bad("Une inspiration est une image."));
    }
    if e.drawer == "ingredient" && e.kind == "image" {
        return Err(bad(
            "Un ingrédient est une police, un cadre, un effet ou un fond.",
        ));
    }
    if e.drawer == "ingredient" && e.license.trim().len() < 3 {
        return Err(bad(
            "Un ingrédient doit avoir sa licence (libre de droits ou créé par POP) : indiquez-la.",
        ));
    }
    if e.title.trim().is_empty() || e.title.len() > 120 {
        return Err(bad("Titre : de 1 à 120 caractères."));
    }
    if e.data.is_none() && e.url.as_deref().is_none_or(|u| !u.starts_with("https://")) {
        return Err(bad("Ajoutez un fichier ou un lien https://."));
    }
    Ok(())
}

async fn telecharger(url: &str) -> Result<Vec<u8>, AppError> {
    let r = reqwest::Client::new()
        .get(url)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| bad(format!("Lien illisible : {e}")))?;
    let b = r.bytes().await.map_err(|e| bad(e.to_string()))?;
    if b.len() > MAX_FICHIER {
        return Err(bad("Fichier de plus de 10 Mo."));
    }
    Ok(b.to_vec())
}

async fn envoyer(s3: &S3, key: &str, octets: Vec<u8>, mime: &str) -> Result<(), AppError> {
    reqwest::Client::new()
        .put(s3.presign("PUT", key, 900, chrono::Utc::now()))
        .header("content-type", mime)
        .body(octets)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| bad(format!("Envoi au stockage objet impossible : {e}")))?;
    Ok(())
}

async fn lire(s3: &S3, key: &str) -> Result<Vec<u8>, AppError> {
    let r = reqwest::get(s3.presign("GET", key, 300, chrono::Utc::now()))
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| bad(format!("Lecture du stockage objet impossible : {e}")))?;
    Ok(r.bytes().await.map_err(|e| bad(e.to_string()))?.to_vec())
}

fn stockage() -> Result<S3, AppError> {
    S3::from_env().ok_or_else(|| bad("Stockage objet non configuré sur ce serveur POP."))
}

pub async fn ajouter(
    State(st): State<AppState>,
    u: AuthUser,
    Json(e): Json<ElementIn>,
) -> ApiResult<Value> {
    crate::security::require_admin(&st, &u).await?;
    valider(&e)?;
    let octets = match (&e.data, &e.url) {
        (Some(d), _) => {
            use base64::Engine;
            let d = d.rsplit(',').next().unwrap_or(d);
            let v = base64::engine::general_purpose::STANDARD
                .decode(d.trim())
                .map_err(|_| bad("Fichier illisible (base64)."))?;
            if v.len() > MAX_FICHIER {
                return Err(bad("Fichier de plus de 10 Mo."));
            }
            v
        }
        (None, Some(u)) => telecharger(u).await?,
        _ => unreachable!(),
    };
    let (ext, mime, phash) = if e.kind == "police" {
        FontRef::try_from_slice(&octets).map_err(|_| bad("Police illisible (TTF ou OTF)."))?;
        ("ttf", "font/ttf", None)
    } else {
        let img =
            image::load_from_memory(&octets).map_err(|_| bad("Image illisible (PNG ou JPEG)."))?;
        ("png", "image/png", Some(empreinte(&img) as i64))
    };
    let s3 = stockage()?;
    let key = format!("miniatures/kit/{}.{ext}", &random_token()[..24]);
    // Les images sont ré-encodées en PNG (sans métadonnées).
    let octets = if ext == "png" {
        let mut v = std::io::Cursor::new(vec![]);
        image::load_from_memory(&octets)
            .unwrap()
            .write_to(&mut v, image::ImageFormat::Png)
            .map_err(|e| bad(e.to_string()))?;
        v.into_inner()
    } else {
        octets
    };
    let n = octets.len() as i64;
    envoyer(&s3, &key, octets, mime).await?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO thumbnail_kit (drawer, kind, title, keywords, source, license, s3_key, bytes, phash, created_by) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(&e.drawer)
    .bind(&e.kind)
    .bind(e.title.trim())
    .bind(e.keywords.trim().to_lowercase())
    .bind(e.source.trim())
    .bind(e.license.trim())
    .bind(&key)
    .bind(n)
    .bind(phash)
    .bind(u.id)
    .fetch_one(&st.db)
    .await?;
    crate::security::audit(
        &st,
        Some(u.id),
        "miniature.ajout",
        &format!("{} #{id} {}", e.drawer, e.title.trim()),
        "",
    )
    .await;
    Ok(Json(json!({ "id": id })))
}

async fn elements(st: &AppState, mots: &str, tiroir: Option<&str>) -> Result<Vec<Value>, AppError> {
    let rows: Vec<(i64, String, String, String, String, String, String, String, i64)> = sqlx::query_as(
        "SELECT id, drawer, kind, title, keywords, source, license, s3_key, bytes FROM thumbnail_kit WHERE (?1 IS NULL OR drawer = ?1) ORDER BY drawer, id DESC",
    )
    .bind(tiroir)
    .fetch_all(&st.db)
    .await?;
    let s3 = S3::from_env();
    let cherche: Vec<String> = sans_accents(mots)
        .split_whitespace()
        .map(String::from)
        .collect();
    Ok(rows
        .into_iter()
        .filter(|r| cherche.is_empty() || cherche.iter().any(|m| sans_accents(&format!("{} {}", r.3, r.4)).contains(m.as_str())))
        .map(|r| {
            json!({ "id": r.0, "drawer": r.1, "kind": r.2, "title": r.3, "keywords": r.4, "source": r.5, "license": r.6, "bytes": r.8,
                // Liens qui expirent vite (10 min).
                "url": s3.as_ref().map(|s| s.presign("GET", &r.7, 600, chrono::Utc::now())) })
        })
        .collect())
}

pub async fn liste(State(st): State<AppState>, u: AuthUser) -> ApiResult<Value> {
    crate::security::require_admin(&st, &u).await?;
    Ok(Json(json!(elements(&st, "", None).await?)))
}

pub async fn retirer(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Value> {
    crate::security::require_admin(&st, &u).await?;
    sqlx::query("DELETE FROM thumbnail_kit WHERE id = ?")
        .bind(id)
        .execute(&st.db)
        .await?;
    crate::security::audit(&st, Some(u.id), "miniature.retrait", &format!("#{id}"), "").await;
    Ok(Json(json!({ "ok": true })))
}

// ---------- Outils MCP ----------

/// `get_thumbnail_kit` : éléments de la bibliothèque par mots-clés.
pub async fn kit(st: &AppState, a: &Value) -> Result<Value, AppError> {
    let tiroir = a["drawer"].as_str().filter(|d| TIROIRS.contains(d));
    let l = elements(st, a["keywords"].as_str().unwrap_or(""), tiroir).await?;
    Ok(json!({
        "items": l.into_iter().take(30).collect::<Vec<_>>(),
        "rules": "Inspirations : à regarder seulement (couleurs, placement du titre, style), jamais à recopier ni à mettre dans l'image. Ingrédients : utilisables dans compose_thumbnail (font, overlays).",
    }))
}

/// `compose_thumbnail` : photos de l'île + description → miniature et logo, contrôlés, rangés avec la map.
pub async fn composer_outil(
    st: &AppState,
    mid: i64,
    uid: i64,
    a: &Value,
) -> Result<Value, AppError> {
    let titre = a["title"].as_str().unwrap_or("");
    let sous = a["subtitle"].as_str();
    verifier_texte(&format!("{titre} {}", sous.unwrap_or(""))).map_err(bad)?;
    let s3 = stockage()?;
    // Fond : la photo de l'île choisie (#306), ou l'image fournie par l'IA du client.
    let fond = if let Some(b) = a["background_png_base64"].as_str() {
        use base64::Engine;
        let v = base64::engine::general_purpose::STANDARD
            .decode(b.rsplit(',').next().unwrap_or(b).trim())
            .map_err(|_| bad("Fond illisible (base64)."))?;
        image::load_from_memory(&v).map_err(|_| bad("Fond illisible (PNG ou JPEG)."))?
    } else {
        let id = a["capture"].as_i64().ok_or_else(|| bad("Indiquez capture (une photo de l'île prise avec capture_island) ou background_png_base64."))?;
        let key: String =
            sqlx::query_scalar("SELECT s3_key FROM island_captures WHERE id = ? AND map_id = ?")
                .bind(id)
                .bind(mid)
                .fetch_optional(&st.db)
                .await?
                .ok_or_else(|| bad("Photo de l'île introuvable sur cette map."))?;
        image::load_from_memory(&lire(&s3, &key).await?)
            .map_err(|_| bad("Photo de l'île illisible."))?
    };
    let ingredient = |id: i64, sortes: &'static [&'static str]| {
        let st = st.clone();
        async move {
            sqlx::query_scalar::<_, String>("SELECT s3_key FROM thumbnail_kit WHERE id = ? AND drawer = 'ingredient' AND kind IN (SELECT value FROM json_each(?))")
                .bind(id)
                .bind(json!(sortes).to_string())
                .fetch_optional(&st.db)
                .await?
                .ok_or_else(|| bad(format!("Ingrédient #{id} introuvable (seuls les ingrédients de la bibliothèque peuvent entrer dans l'image).")))
        }
    };
    let mut calques = vec![];
    for id in a["overlays"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_i64)
        .take(4)
    {
        let k = ingredient(id, &["cadre", "effet", "fond"]).await?;
        calques.push(
            image::load_from_memory(&lire(&s3, &k).await?)
                .map_err(|_| bad("Ingrédient illisible."))?,
        );
    }
    let police_perso = match a["font"].as_i64() {
        Some(id) => Some(lire(&s3, &ingredient(id, &["police"]).await?).await?),
        None => None,
    };
    let police = match &police_perso {
        Some(v) => FontRef::try_from_slice(v).map_err(|_| bad("Police illisible."))?,
        None => FontRef::try_from_slice(POLICE).unwrap(),
    };
    let d = Demande {
        titre,
        sous_titre: sous,
        couleur: a["title_color"].as_str(),
        contour: a["outline_color"].as_str(),
        position: a["position"].as_str(),
    };
    let (img, logo) = composer(&fond, &calques, &d, &police).map_err(bad)?;
    // Ressemblance avec une inspiration : le fond fourni et l'image finale.
    let inspirations: Vec<(String, i64)> = sqlx::query_as(
        "SELECT title, phash FROM thumbnail_kit WHERE drawer = 'inspiration' AND phash IS NOT NULL",
    )
    .fetch_all(&st.db)
    .await?;
    let insp: Vec<(String, u64)> = inspirations
        .into_iter()
        .map(|(t, h)| (t, h as u64))
        .collect();
    for e in [
        empreinte(&fond),
        empreinte(&DynamicImage::ImageRgba8(img.clone())),
    ] {
        if let Some(t) = trop_proche(e, &insp) {
            return Err(bad(format!("Image trop proche de l'inspiration « {t} » : inspirez-vous-en sans la recopier (autre fond, autre composition).")));
        }
    }
    let ((octets, ext), logo_png) = encoder(&img, &logo).map_err(bad)?;
    controler(
        &DynamicImage::ImageRgba8(img),
        octets.len(),
        &image::load_from_memory(&logo_png).map_err(|e| bad(e.to_string()))?,
    )
    .map_err(bad)?;
    let base = format!("miniatures/maps/{mid}/{}", &random_token()[..20]);
    let (km, kl) = (format!("{base}.{ext}"), format!("{base}-logo.png"));
    let n = octets.len() as i64;
    envoyer(
        &s3,
        &km,
        octets,
        if ext == "png" {
            "image/png"
        } else {
            "image/jpeg"
        },
    )
    .await?;
    envoyer(&s3, &kl, logo_png, "image/png").await?;
    let id: i64 = sqlx::query_scalar("INSERT INTO map_thumbnails (map_id, title, s3_key, logo_key, bytes, created_by) VALUES (?, ?, ?, ?, ?, ?) RETURNING id")
        .bind(mid)
        .bind(titre.trim())
        .bind(&km)
        .bind(&kl)
        .bind(n)
        .bind(uid)
        .fetch_one(&st.db)
        .await?;
    Ok(json!({
        "id": id,
        "thumbnail": { "url": s3.presign("GET", &km, 3600, chrono::Utc::now()), "width": LARGEUR, "height": HAUTEUR, "bytes": n },
        "logo": { "url": s3.presign("GET", &kl, 3600, chrono::Utc::now()), "width": LOGO_L, "height": LOGO_H },
        "checks": "1920 × 1080, moins de 5 Mo, logo transparent 1440 × 608, pas de mot interdit ni de commande, pas trop proche d'une inspiration.",
    }))
}

// ---------- « Générer une miniature » dans Forge (#308) ----------

#[derive(Deserialize)]
pub struct DemandeIn {
    /// « Une idée en plus ? » (facultatif).
    #[serde(default)]
    idee: Option<String>,
    /// « Modifier » : la miniature de départ et la phrase de changement.
    #[serde(default)]
    base: Option<i64>,
    /// Images d'exemple (data URL ou base64 PNG/JPEG), 4 au plus (#468).
    #[serde(default)]
    exemples: Vec<String>,
}

/// L'IA connectée de la personne sur cette map (la plus récente, active depuis moins d'un jour).
async fn ia_connectee(
    st: &AppState,
    mid: i64,
    uid: i64,
) -> Result<Option<(i64, String)>, AppError> {
    Ok(sqlx::query_as(
        "SELECT id, name FROM api_tokens WHERE map_id = ? AND user_id = ? AND scope = 'write' AND last_used > datetime('now', '-1 day') ORDER BY last_used DESC LIMIT 1",
    )
    .bind(mid)
    .bind(uid)
    .fetch_optional(&st.db)
    .await?)
}

/// Consigne de la tâche donnée à l'IA du client.
pub fn consigne(idee: &str, base: Option<(i64, &str)>, exemples: &[String]) -> String {
    let mut c = String::from("Fais la miniature de cette map, en suivant la compétence « miniature » (get_skill) :\n1. capture_island : 2 ou 3 vraies photos de l'île qui montrent le gameplay.\n2. get_thumbnail_kit : regarde des inspirations du même genre (sans les recopier).\n3. compose_thumbnail avec la meilleure photo et un titre court ; si POP refuse, corrige et recommence.\n4. submit_thumbnail avec l'id rendu et cette tâche, puis passe la tâche en done. Pour parler à l'humain pendant ce travail : send_message avec canal « miniature » (il est dans l'outil Miniature, pas dans votre terminal).\nUne seule miniature finie : l'humain la choisit dans Forge.");
    if let Some((id, titre)) = base {
        c.push_str(&format!("\n\nModification de la miniature #{id} (« {titre} ») : garde la même photo et le même style, change seulement ce que demande l'humain."));
    }
    if !exemples.is_empty() {
        c.push_str("\n\nImages d'exemple de l'humain (à regarder pour comprendre ce qu'il veut, jamais à recopier ni à mettre dans l'image) :");
        for e in exemples {
            c.push_str(&format!("\n- {e}"));
        }
    }
    if !idee.trim().is_empty() {
        c.push_str(&format!("\n\nIdée de l'humain : « {} ». Si elle demande quelque chose d'interdit par Epic (V-Bucks, XP, nom de Fortnite…), ne le mets pas et fais au plus proche sans le lui redemander.", idee.trim()));
    }
    c
}

/// `POST /api/maps/{mid}/miniatures/demandes` : crée la tâche pour l'IA du client et la lui attribue si une IA
/// travaille sur la map ; elle reçoit aussi un message (joint à sa prochaine réponse, #464).
pub async fn demander(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
    Json(b): Json<DemandeIn>,
) -> ApiResult<Value> {
    crate::billing::require_forge(&st, u.id).await?;
    let ctx = crate::access::map_ctx(&st, &u, mid).await?;
    ctx.can_edit()?;
    let idee: String = b
        .idee
        .unwrap_or_default()
        .trim()
        .chars()
        .take(300)
        .collect();
    let base = match b.base {
        Some(id) => Some(
            sqlx::query_as::<_, (i64, String)>(
                "SELECT id, title FROM map_thumbnails WHERE id = ? AND map_id = ?",
            )
            .bind(id)
            .bind(mid)
            .fetch_optional(&st.db)
            .await?
            .ok_or(AppError::NotFound)?,
        ),
        None => None,
    };
    if base.is_some() && idee.is_empty() {
        return Err(bad("Dites en une phrase ce qu'il faut changer."));
    }
    // L'outil parle à l'IA de la personne : sans IA connectée à la map, rien n'est envoyé (#468).
    let (aid, nom) = ia_connectee(&st, mid, u.id).await?.ok_or_else(|| {
        bad("Aucune IA connectée à cette map : lancez-en une depuis Agents → + Terminal, ou connectez la vôtre (POP → IA connectées).")
    })?;
    if b.exemples.len() > 4 {
        return Err(bad("4 images d'exemple au plus."));
    }
    let mut cles = vec![];
    let mut liens = vec![];
    if !b.exemples.is_empty() {
        let s3 = stockage()?;
        for e in &b.exemples {
            use base64::Engine;
            let v = base64::engine::general_purpose::STANDARD
                .decode(e.rsplit(',').next().unwrap_or(e).trim())
                .map_err(|_| bad("Image d'exemple illisible."))?;
            if v.len() > 5 * 1024 * 1024 {
                return Err(bad("Image d'exemple de plus de 5 Mo."));
            }
            let img = image::load_from_memory(&v)
                .map_err(|_| bad("Image d'exemple illisible (PNG ou JPEG)."))?;
            let mut png = std::io::Cursor::new(vec![]);
            img.write_to(&mut png, image::ImageFormat::Png)
                .map_err(|e| bad(e.to_string()))?;
            let k = format!("miniatures/demandes/{mid}/{}.png", &random_token()[..20]);
            envoyer(&s3, &k, png.into_inner(), "image/png").await?;
            // Lien valable 7 jours (le maximum d'un lien signé) : le temps que l'IA prenne la tâche.
            liens.push(s3.presign("GET", &k, 7 * 24 * 3600, chrono::Utc::now()));
            cles.push(k);
        }
    }
    let t = crate::tasks::NewTask {
        title: if base.is_some() {
            "Miniature : modifier".into()
        } else {
            "Miniature de la map".into()
        },
        tag: Some("conception".into()),
        description: Some(consigne(
            &idee,
            base.as_ref().map(|(i, t)| (*i, t.as_str())),
            &liens,
        )),
        assignee_id: None,
        deps: None,
        planned_day: None,
        lot: None,
        status: None,
        ai: Some(true),
        origin: Some("miniature".into()),
        uefn: None,
    };
    let (tid, number) = crate::tasks::insert_task(&st, &ctx, &t).await?;
    sqlx::query("INSERT INTO thumbnail_requests (map_id, task_id, idea, base_id, created_by, examples) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(mid)
        .bind(tid)
        .bind(&idee)
        .bind(base.as_ref().map(|b| b.0))
        .bind(u.id)
        .bind(json!(cles).to_string())
        .execute(&st.db)
        .await?;
    sqlx::query("UPDATE tasks SET agent_id = ? WHERE id = ?")
        .bind(aid)
        .bind(tid)
        .execute(&st.db)
        .await?;
    // Le message de l'humain, tel quel, dans la discussion avec son IA (#468).
    let corps = if idee.is_empty() {
        "Fais la miniature de la map.".to_string()
    } else {
        idee.clone()
    };
    let pj = if cles.is_empty() {
        String::new()
    } else {
        format!(" {} image(s) d'exemple dans la tâche.", cles.len())
    };
    sqlx::query("INSERT INTO agent_messages (map_id, agent_id, user_id, body, canal) VALUES (?, ?, ?, ?, 'miniature')")
        .bind(mid)
        .bind(aid)
        .bind(u.id)
        .bind(format!("{corps}\n(Outil Miniature : tâche #{number}, get_task {number}, puis submit_thumbnail ; répondez avec send_message canal « miniature ».{pj})"))
        .execute(&st.db)
        .await?;
    crate::access::notify(&st, mid, json!({ "t": "refresh", "what": "tasks" }));
    crate::access::notify(&st, mid, json!({ "t": "refresh", "what": "messages" }));
    Ok(Json(
        json!({ "task": number, "agent": nom, "agent_id": aid }),
    ))
}

/// `GET /api/maps/{mid}/miniatures` : miniatures rendues par l'IA (la plus récente d'abord) et demandes en cours.
pub async fn de_la_map(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
) -> ApiResult<Value> {
    crate::access::map_ctx(&st, &u, mid).await?;
    let rows: Vec<(i64, String, String, String, i64, Option<String>, Option<String>, String)> = sqlx::query_as(
        "SELECT id, title, s3_key, logo_key, bytes, submitted_at, used_at, created_at FROM map_thumbnails WHERE map_id = ? AND submitted_at IS NOT NULL ORDER BY id DESC LIMIT 20",
    )
    .bind(mid)
    .fetch_all(&st.db)
    .await?;
    let s3 = S3::from_env();
    let lien = |k: &str| {
        s3.as_ref()
            .map(|s| s.presign("GET", k, 3600, chrono::Utc::now()))
    };
    let en_cours: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT t.number, t.status, r.idea FROM thumbnail_requests r JOIN tasks t ON t.id = r.task_id WHERE r.map_id = ? AND t.status <> 'done'
           AND NOT EXISTS (SELECT 1 FROM map_thumbnails m WHERE m.task_id = r.task_id AND m.submitted_at IS NOT NULL) ORDER BY r.id DESC",
    )
    .bind(mid)
    .fetch_all(&st.db)
    .await?;
    let ia = ia_connectee(&st, mid, u.id).await?;
    Ok(Json(json!({
        "ia": ia.map(|(id, name)| json!({ "id": id, "name": name })),
        "miniatures": rows.iter().map(|r| json!({ "id": r.0, "title": r.1, "url": lien(&r.2), "logo": lien(&r.3), "bytes": r.4, "submitted_at": r.5, "used_at": r.6, "created_at": r.7 })).collect::<Vec<_>>(),
        "en_cours": en_cours.iter().map(|r| json!({ "task": r.0, "status": r.1, "idea": r.2 })).collect::<Vec<_>>(),
    })))
}

/// `POST /api/maps/{mid}/miniatures/{id}/utiliser` : la miniature choisie pour la map (versions gardées).
pub async fn utiliser(
    State(st): State<AppState>,
    u: AuthUser,
    Path((mid, id)): Path<(i64, i64)>,
) -> ApiResult<Value> {
    let ctx = crate::access::map_ctx(&st, &u, mid).await?;
    ctx.can_edit()?;
    let n = sqlx::query("UPDATE map_thumbnails SET used_at = datetime('now') WHERE id = ? AND map_id = ? AND submitted_at IS NOT NULL")
        .bind(id)
        .bind(mid)
        .execute(&st.db)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(AppError::NotFound);
    }
    crate::access::notify(&st, mid, json!({ "t": "refresh", "what": "miniatures" }));
    Ok(Json(json!({ "ok": true })))
}

/// `submit_thumbnail { id, task }` : l'IA rend la miniature finie à l'humain (Forge la lui montre).
pub async fn rendre(st: &AppState, mid: i64, a: &Value) -> Result<Value, AppError> {
    let id = a["id"]
        .as_i64()
        .ok_or_else(|| bad("id manquant : celui rendu par compose_thumbnail."))?;
    let tid: Option<i64> = match a["task"].as_i64() {
        Some(n) => {
            sqlx::query_scalar("SELECT id FROM tasks WHERE map_id = ? AND number = ?")
                .bind(mid)
                .bind(n)
                .fetch_optional(&st.db)
                .await?
        }
        None => None,
    };
    let n = sqlx::query("UPDATE map_thumbnails SET submitted_at = datetime('now'), task_id = COALESCE(?, task_id) WHERE id = ? AND map_id = ?")
        .bind(tid)
        .bind(id)
        .bind(mid)
        .execute(&st.db)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(bad(
            "Miniature introuvable sur cette map : utilisez l'id rendu par compose_thumbnail.",
        ));
    }
    crate::access::notify(st, mid, json!({ "t": "refresh", "what": "miniatures" }));
    Ok(
        json!({ "ok": true, "message": "Miniature montrée à l'humain dans Forge. Passez la tâche en done." }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn photo() -> DynamicImage {
        // Une « photo » : un dégradé avec du bruit, comme une vraie capture (compression réaliste).
        DynamicImage::ImageRgba8(RgbaImage::from_fn(1920, 1080, |x, y| {
            let n = ((x * 7919 + y * 104729) % 37) as u8;
            Rgba([(x / 8) as u8 ^ n, (y / 5) as u8, 120u8.wrapping_add(n), 255])
        }))
    }

    #[test]
    fn consigne_de_la_tache() {
        let c = consigne("ajoute des V-Bucks", None, &["https://r2/ex.png".into()]);
        assert!(c.contains("https://r2/ex.png") && c.contains("jamais à recopier"));
        assert!(
            c.contains("capture_island")
                && c.contains("submit_thumbnail")
                && c.contains("interdit par Epic"),
            "{c}"
        );
        assert!(consigne("", Some((3, "Course")), &[]).contains("#3"));
    }

    #[test]
    fn mots_interdits() {
        let e = verifier_texte("AFK XP illimité").unwrap_err();
        assert!(e.contains("AFK") && e.contains("XP"), "{e}");
        let e = verifier_texte("Gagne des V-Bucks").unwrap_err();
        assert!(
            e.contains("« VBUCKS »") && !e.contains("VBUCK »") && e.matches('«').count() == 1,
            "{e}"
        );
        assert!(verifier_texte("pièces gratuites").is_err());
        assert!(verifier_texte("Appuie sur X")
            .unwrap_err()
            .contains("commandes"));
        assert!(verifier_texte("Course des toits").is_ok());
        assert!(
            verifier_texte("Expédition").is_ok(),
            "XP dans un mot n'est pas « XP »"
        );
    }

    #[test]
    fn dimensions_poids_et_logo() {
        let police = FontRef::try_from_slice(POLICE).unwrap();
        let d = Demande {
            titre: "Course des toits",
            sous_titre: Some("Saute, glisse, gagne"),
            couleur: None,
            contour: None,
            position: Some("bas"),
        };
        let (img, logo) = composer(&photo(), &[], &d, &police).unwrap();
        let ((m, _), l) = encoder(&img, &logo).unwrap();
        assert!(m.len() <= MAX_OCTETS, "{} octets", m.len());
        let im = image::load_from_memory(&m).unwrap();
        let lo = image::load_from_memory(&l).unwrap();
        assert_eq!(im.dimensions(), (1920, 1080));
        assert_eq!(lo.dimensions(), (1440, 608));
        assert!(
            lo.color().has_alpha() && lo.get_pixel(0, 0)[3] == 0,
            "logo transparent"
        );
        assert!(
            lo.to_rgba8().pixels().any(|p| p[3] == 255),
            "le titre est dessiné"
        );
        controler(&im, m.len(), &lo).unwrap();
        assert!(composer(
            &photo(),
            &[],
            &Demande {
                couleur: Some("violet"),
                ..d
            },
            &police
        )
        .is_err());
    }

    #[test]
    fn trop_proche_d_une_inspiration() {
        let insp = photo();
        let base = vec![("Guess the number".to_string(), empreinte(&insp))];
        // La même image, légèrement retouchée : refusée.
        let mut copie = insp.to_rgba8();
        for p in copie.pixels_mut() {
            p[0] = p[0].saturating_add(6);
        }
        assert_eq!(
            trop_proche(empreinte(&DynamicImage::ImageRgba8(copie)), &base).as_deref(),
            Some("Guess the number")
        );
        // Une image différente : acceptée.
        // Une autre composition (image retournée, autre cadrage) : acceptée.
        let autre = insp.fliph().crop_imm(300, 0, 1200, 1080);
        assert!(trop_proche(empreinte(&autre), &base).is_none());
    }

    #[test]
    fn ingredient_sans_licence() {
        let e = |drawer: &str, kind: &str, license: &str| ElementIn {
            drawer: drawer.into(),
            kind: kind.into(),
            title: "Cadre néon".into(),
            keywords: String::new(),
            source: String::new(),
            license: license.into(),
            data: Some("AAAA".into()),
            url: None,
        };
        assert!(valider(&e("ingredient", "cadre", "")).is_err());
        assert!(valider(&e("ingredient", "cadre", "CC0")).is_ok());
        assert!(
            valider(&e("inspiration", "image", "")).is_ok(),
            "une inspiration n'entre jamais dans l'image"
        );
        assert!(valider(&e("inspiration", "cadre", "")).is_err());
    }

    /// #308 : « Générer une miniature » crée la tâche pour l'IA qui travaille sur la map (avec un message) ;
    /// submit_thumbnail la montre dans Forge ; « Utiliser » la choisit ; « Modifier » demande une phrase.
    #[tokio::test]
    async fn generer_rendre_utiliser() {
        let st = crate::test_state().await;
        let q = |sql: &'static str| sqlx::query_scalar::<_, i64>(sql);
        let uid = q("INSERT INTO users (email, name, password_hash, email_verified_at, is_admin) VALUES ('kl@a.fr', 'KL', 'x', datetime('now'), 1) RETURNING id").fetch_one(&st.db).await.unwrap();
        let sid = q("INSERT INTO studios (name) VALUES ('S') RETURNING id")
            .fetch_one(&st.db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO memberships (studio_id, user_id, role) VALUES (?, ?, 'owner')")
            .bind(sid)
            .bind(uid)
            .execute(&st.db)
            .await
            .unwrap();
        let mid: i64 = sqlx::query_scalar(
            "INSERT INTO maps (studio_id, name) VALUES (?, 'GrassCutter') RETURNING id",
        )
        .bind(sid)
        .fetch_one(&st.db)
        .await
        .unwrap();
        let u0 = AuthUser {
            id: uid,
            email: "kl@a.fr".into(),
            name: "KL".into(),
            sid: String::new(),
        };
        // Sans IA connectée : refus, rien n'est créé (#468).
        let e = demander(
            State(st.clone()),
            u0,
            Path(mid),
            Json(DemandeIn {
                idee: Some("fun".into()),
                base: None,
                exemples: vec![],
            }),
        )
        .await
        .unwrap_err();
        assert!(format!("{e:?}").contains("Aucune IA connectée"));
        let ag: i64 = sqlx::query_scalar("INSERT INTO api_tokens (user_id, map_id, name, token_hash, scope, last_used) VALUES (?, ?, 'Claude', 'h', 'write', datetime('now')) RETURNING id")
            .bind(uid).bind(mid).fetch_one(&st.db).await.unwrap();
        let u = AuthUser {
            id: uid,
            email: "kl@a.fr".into(),
            name: "KL".into(),
            sid: String::new(),
        };
        let d = |idee: &str, base: Option<i64>| {
            Json(DemandeIn {
                idee: Some(idee.into()),
                base,
                exemples: vec![],
            })
        };
        let Json(r) = demander(
            State(st.clone()),
            u.clone(),
            Path(mid),
            d("ambiance fun, ajoute des V-Bucks", None),
        )
        .await
        .unwrap();
        assert_eq!(r["agent"], "Claude");
        let n = r["task"].as_i64().unwrap();
        let (desc, agent): (String, Option<i64>) = sqlx::query_as(
            "SELECT description, agent_id FROM tasks WHERE map_id = ? AND number = ?",
        )
        .bind(mid)
        .bind(n)
        .fetch_one(&st.db)
        .await
        .unwrap();
        assert_eq!(agent, Some(ag));
        assert!(
            desc.contains("V-Bucks") && desc.contains("ne le mets pas"),
            "l'idée interdite est corrigée par l'IA sans redemander"
        );
        assert_eq!(
            crate::messages::non_lus(&st, ag).await.unwrap(),
            1,
            "l'IA est prévenue"
        );
        let Json(l) = de_la_map(State(st.clone()), u.clone(), Path(mid))
            .await
            .unwrap();
        assert_eq!(l["en_cours"][0]["task"], n);
        // L'IA a composé (#307), puis rend la miniature.
        let tid: i64 = sqlx::query_scalar("INSERT INTO map_thumbnails (map_id, title, s3_key, logo_key, bytes) VALUES (?, 'Course des toits', 'k', 'l', 10) RETURNING id").bind(mid).fetch_one(&st.db).await.unwrap();
        rendre(&st, mid, &json!({ "id": tid, "task": n }))
            .await
            .unwrap();
        assert!(rendre(&st, mid, &json!({ "id": 999 })).await.is_err());
        let Json(l) = de_la_map(State(st.clone()), u.clone(), Path(mid))
            .await
            .unwrap();
        assert_eq!(l["miniatures"][0]["id"], tid);
        assert!(l["en_cours"].as_array().unwrap().is_empty());
        utiliser(State(st.clone()), u.clone(), Path((mid, tid)))
            .await
            .unwrap();
        let Json(l) = de_la_map(State(st.clone()), u.clone(), Path(mid))
            .await
            .unwrap();
        assert!(l["miniatures"][0]["used_at"].is_string());
        // « Modifier » sans phrase : refus ; avec : nouvelle tâche qui part de cette miniature.
        assert!(
            demander(State(st.clone()), u.clone(), Path(mid), d("", Some(tid)))
                .await
                .is_err()
        );
        let Json(r2) = demander(
            State(st.clone()),
            u.clone(),
            Path(mid),
            d("titre plus gros", Some(tid)),
        )
        .await
        .unwrap();
        let desc: String =
            sqlx::query_scalar("SELECT description FROM tasks WHERE map_id = ? AND number = ?")
                .bind(mid)
                .bind(r2["task"].as_i64().unwrap())
                .fetch_one(&st.db)
                .await
                .unwrap();
        assert!(desc.contains(&format!("#{tid}")) && desc.contains("titre plus gros"));
    }

    /// La bibliothèque est réservée à l'équipe POP : un client reçoit un refus.
    #[tokio::test]
    async fn reservee_a_l_equipe_pop() {
        let st = crate::test_state().await;
        let id: i64 = sqlx::query_scalar("INSERT INTO users (email, name, password_hash, email_verified_at) VALUES ('client@a.fr', 'Client', 'x', datetime('now')) RETURNING id")
            .fetch_one(&st.db)
            .await
            .unwrap();
        let u = AuthUser {
            id,
            email: "client@a.fr".into(),
            name: "Client".into(),
            sid: String::new(),
        };
        assert!(matches!(
            liste(State(st.clone()), u.clone()).await,
            Err(AppError::Forbidden)
        ));
        assert!(matches!(
            retirer(State(st.clone()), u, Path(1)).await,
            Err(AppError::Forbidden)
        ));
    }
}
