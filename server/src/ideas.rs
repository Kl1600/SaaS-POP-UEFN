//! Idées de maps.
//!
//! 1. Le marché est découpé en **niches** : un thème ou une mécanique (tycoon, prop hunt, 1v1, horreur…),
//!    reconnu dans le titre et les tags de chaque île, rattaché à l'un des 12 genres officiels de Discover.
//!    Les mots qui montent dans les titres des maps qui marchent deviennent aussi des niches (« émergentes »).
//! 2. Chaque niche est notée sur 5 axes, en centiles par rapport aux autres niches :
//!    - demande : joueurs, joueurs d'une map typique ;
//!    - fidélité : temps de jeu, retour à J+1 et J+7, recommandations par joueur (les signaux qu'Epic note par joueur) ;
//!    - porte ouverte : part des maps sorties depuis 30 jours qui ont dépassé 100 / 1 000 joueurs, poids des 3 premières ;
//!    - place à prendre : beaucoup de joueurs mais des leaders au temps de jeu ou à la rétention faibles ;
//!    - élan : progression sur 7 (et 14) jours, mentions dans les vidéos TikTok / YouTube récentes.
//! 3. Les 12 meilleures niches deviennent des idées (les mêmes pour tous), avec alertes quand une nouvelle arrive.
use crate::{auth::AuthUser, error::{bad, ApiResult, AppError}, AppState};
use axum::{extract::{Path, State}, Json};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

/// (clé, libellé fr, libellé en, genre, complexité 1-3, compétences, motifs, drapeau)
/// Drapeau : "" | "risque" (exclu des idées : maps XP/AFK, pénalisées par Epic) | "noel" | "halloween" | "ete" (saisonnier).
const THEMES: &[(&str, &str, &str, &str, u8, &str, &str, &str)] = &[
    ("tycoon", "Tycoon", "Tycoon", "simulation", 2, "verse,build", "tycoon,tycoons", ""),
    ("simulator", "Simulateur", "Simulator", "simulation", 2, "verse,design", "simulator,simulateur", ""),
    ("brainrot", "Brainrot", "Brainrot", "simulation", 2, "verse,art", "brainrot,brainrots", ""),
    ("steal", "Vol d’objets (steal a…)", "Steal a…", "simulation", 2, "verse,design", "steal a,steal the,steal", ""),
    ("rng", "Chance et tirages (RNG)", "RNG and luck", "simulation", 2, "verse,design", "rng,gacha,lucky block,lucky blocks,spin,loot", ""),
    ("merge", "Fusion (merge)", "Merge", "simulation", 2, "verse,design", "merge", ""),
    ("clicker", "Clicker et idle", "Clicker and idle", "simulation", 1, "verse", "clicker,idle,incremental", ""),
    ("pets", "Animaux de compagnie", "Pets", "simulation", 2, "verse,art", "pet,pets,animal,animals", ""),
    ("farming", "Ferme", "Farming", "simulation", 2, "verse,build", "farm,farming,ferme,garden", ""),
    ("fishing", "Pêche", "Fishing", "simulation", 2, "verse,build", "fishing,fish,peche", ""),
    ("cooking", "Cuisine et restaurant", "Cooking and restaurant", "simulation", 2, "verse,build", "cook,cooking,restaurant,cafe,pizza,burger", ""),
    ("mining", "Mine", "Mining", "simulation", 2, "verse,build", "mining,miner,mine", ""),
    ("horror", "Horreur", "Horror", "horreur", 3, "build,art,design", "horror,horreur,scary,haunted,backrooms,asylum,creepy,nightmare,terror", ""),
    ("escape", "Évasion (escape)", "Escape", "aventure", 2, "build,design", "escape,evasion,escape room", ""),
    ("puzzle", "Énigmes et labyrinthes", "Puzzles and mazes", "aventure", 2, "build,design", "puzzle,puzzles,maze,labyrinth,riddle", ""),
    ("rpg", "RPG et quêtes", "RPG and quests", "aventure", 3, "verse,build,art,design", "rpg,quest,quests,dungeon,dungeons,adventure,aventure,boss,bosses,raid", ""),
    ("anime", "Anime et ninjas", "Anime and ninjas", "aventure", 2, "verse,art", "anime,ninja,ninjas,samurai,shinobi", ""),
    ("hero", "Super-héros et pouvoirs", "Superheroes and powers", "aventure", 2, "verse,art", "superhero,superheroes,super hero,hero,heroes,powers,super power", ""),
    ("monsters", "Monstres et dinosaures", "Monsters and dinosaurs", "aventure", 2, "verse,art", "monster,monsters,dino,dinosaur,dinosaurs,dragon,dragons", ""),
    ("deathrun", "Deathrun", "Deathrun", "deathrun", 1, "build,design", "deathrun,death run,deathruns", ""),
    ("parkour", "Parkour et obby", "Parkour and obby", "deathrun", 1, "build,design", "parkour,obby,only up,onlyup,climb,jump", ""),
    ("prophunt", "Prop hunt et cache-cache", "Prop hunt and hide and seek", "fete", 2, "verse,build", "prop hunt,prophunt,hide and seek,hide n seek,hide seek,cache cache", ""),
    ("murder", "Murder mystery et imposteur", "Murder mystery and impostor", "fete", 2, "verse,design", "murder,mystery,impostor,imposter,traitor", ""),
    ("party", "Mini-jeux et fête", "Minigames and party", "fete", 2, "verse,design", "minigame,minigames,mini game,mini games,party,mini jeux", ""),
    ("lava", "Sol de lave et catastrophes", "Floor is lava and disasters", "fete", 1, "verse,build", "lava,tsunami,flood,disaster,disasters,floor is lava", ""),
    ("tag", "Chat et poursuite", "Tag and chase", "fete", 1, "verse,build", "freeze tag,tag game,chase,infection,infected", ""),
    ("zonewars", "Zone wars", "Zone wars", "tir", 1, "build,design", "zone wars,zonewars,zone war", ""),
    ("boxfight", "Box fight et build fight", "Box fights and build fights", "tir", 1, "build", "box fight,boxfight,box fights,boxfights,build fight,buildfight,build fights", ""),
    ("duel", "Duels (1v1, 2v2…)", "Duels (1v1, 2v2…)", "tir", 1, "build", "1v1,2v2,3v3,4v4,1vs1,2vs2,1v1v1", ""),
    ("redvsblue", "Rouges contre bleus", "Red vs blue", "tir", 1, "build", "red vs blue,red v blue,rvb", ""),
    ("gungame", "Gun game et one shot", "Gun game and one shot", "tir", 1, "verse,build", "gun game,gungame,one shot,oneshot,1 shot", ""),
    ("sniper", "Snipers", "Snipers", "tir", 1, "build", "sniper,snipers", ""),
    ("ffa", "Chacun pour soi (FFA)", "Free for all", "tir", 1, "build", "ffa,free for all", ""),
    ("tactical", "Tir tactique (TDM, domination)", "Tactical shooters", "tir", 2, "verse,build,design", "tactical,tdm,team deathmatch,domination,search and destroy,realistic,realistic", ""),
    ("war", "Guerre et militaire", "War and military", "tir", 2, "verse,build,art", "war,military,army,ww2,soldier,soldiers", ""),
    ("training", "Entraînement (aim, edit)", "Training (aim, edits)", "tir", 1, "build", "aim,aim training,edit course,edit map,edits,practice,training,piece control,warmup", ""),
    ("br", "Battle royale", "Battle royale", "battle_royale", 2, "verse,build,design", "battle royale,lategame,late game,endgame,scrims,scrim", ""),
    ("zombie", "Zombies", "Zombies", "survie", 2, "verse,build", "zombie,zombies,undead", ""),
    ("survival", "Survie", "Survival", "survie", 3, "verse,build,design", "survival,survive,survie,stranded", ""),
    ("towerdef", "Tower defense", "Tower defense", "strategie", 3, "verse,design", "tower defense,tower defence", ""),
    ("bedwars", "Bed wars et sky wars", "Bed wars and sky wars", "strategie", 2, "verse,build", "bedwars,bed wars,skywars,sky wars,skyblock", ""),
    ("strategy", "Stratégie et cartes", "Strategy and cards", "strategie", 3, "verse,design", "strategy,rts,card game,cards,chess", ""),
    ("roleplay", "Jeu de rôle (RP)", "Roleplay", "social", 2, "build,art", "roleplay,role play,rp,city rp,life,prison,school,hotel,brookhaven", ""),
    ("hangout", "Lieu de rencontre", "Hangout", "social", 1, "build,art", "hangout,chill,club,lounge", ""),
    ("music", "Musique et rythme", "Music and rhythm", "musique", 2, "verse,design", "music,rhythm,dance,concert,festival,dj", ""),
    ("racing", "Courses et voitures", "Racing and cars", "sports", 2, "verse,build", "race,racing,drift,cars,car,kart,karts", ""),
    ("soccer", "Football", "Soccer", "sports", 2, "verse,build", "soccer,football,foot", ""),
    ("basketball", "Basket", "Basketball", "sports", 2, "verse,build", "basketball,basket", ""),
    ("othersports", "Autres sports", "Other sports", "sports", 2, "verse,build", "golf,bowling,volleyball,tennis,hockey,baseball", ""),
    ("noel", "Noël et hiver", "Christmas and winter", "fete", 1, "build,art", "christmas,xmas,noel,santa,winter,snow,holiday", "noel"),
    ("halloween", "Halloween", "Halloween", "fete", 1, "build,art", "halloween,pumpkin,spooky", "halloween"),
    ("summer", "Été et plage", "Summer and beach", "fete", 1, "build,art", "summer,beach,pool,water park,waterpark", "ete"),
    ("xp", "Maps XP / AFK", "XP / AFK maps", "simulation", 1, "verse", "xp,afk,xp map,level up fast", "risque"),
];

pub const STOP: &[&str] = &["the", "and", "with", "for", "you", "your", "map", "maps", "fortnite", "new", "best", "mode", "game", "games", "creative", "uefn", "season",
    "chapter", "update", "updated", "all", "edition", "version", "official", "free", "fun", "ultimate", "super", "mega", "epic", "pro", "les", "des", "une", "dans", "pour",
    "avec", "sur", "and", "only", "from", "this", "that", "are", "get", "can", "who", "how", "what", "play", "players", "player", "team", "teams", "world", "island",
    "islands", "code", "codes", "remake", "remastered", "beta", "alpha", "classic", "original", "edition", "lobby", "solo", "duo", "duos", "squad", "squads", "trio", "trios"];

pub struct Theme { pub key: String, pub fr: String, pub en: String, pub genre: &'static str, complexity: u8, skills: Vec<&'static str>, pub flag: &'static str, pub pats: Vec<String>, emerging: bool }

pub fn themes() -> Vec<Theme> {
    THEMES.iter().map(|t| Theme {
        key: t.0.into(), fr: t.1.into(), en: t.2.into(), genre: t.3, complexity: t.4, skills: t.5.split(',').collect(), flag: t.7,
        pats: t.6.split(',').map(|p| format!(" {} ", p.trim())).collect(), emerging: false,
    }).collect()
}

/// Titre + tags en minuscules, sans accents ni ponctuation, entourés d'espaces (pour chercher des mots entiers).
pub fn norm(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push(' ');
    for c in s.to_lowercase().chars() {
        let c = match c { 'à' | 'â' | 'ä' => 'a', 'é' | 'è' | 'ê' | 'ë' => 'e', 'î' | 'ï' => 'i', 'ô' | 'ö' => 'o', 'ù' | 'û' | 'ü' => 'u', 'ç' => 'c', _ => c };
        out.push(if c.is_alphanumeric() { c } else { ' ' });
    }
    out.push(' ');
    // Espaces simples.
    out.split_whitespace().fold(String::from(" "), |mut a, w| { a.push_str(w); a.push(' '); a })
}

/// Évolution d'un groupe de maps : moyenne des 2 derniers jours face aux 2 premiers, sur les mêmes maps
/// (une map apparue en cours de route ne fausse pas la comparaison, un pic d'un seul jour pèse moitié moins).
fn trend_of(ids: &[usize], isl: &[Isl], days: &[String]) -> Option<f64> {
    if days.len() < 4 { return None }
    let (early, late) = (&days[..2], &days[days.len() - 2..]);
    let (mut f, mut l) = (0f64, 0f64);
    for &i in ids {
        let e: Vec<i64> = early.iter().filter_map(|d| isl[i].by_day.get(d).copied()).collect();
        let t: Vec<i64> = late.iter().filter_map(|d| isl[i].by_day.get(d).copied()).collect();
        if e.len() == 2 && t.len() == 2 { f += e.iter().sum::<i64>() as f64 / 2.0; l += t.iter().sum::<i64>() as f64 / 2.0 }
    }
    (f > 0.0).then(|| l / f - 1.0)
}

struct Isl { code: String, title: String, text: String, is_new: bool, by_day: HashMap<String, i64>, best: i64, mins: Option<f64>, d1: Option<f64>, d7: Option<f64>, rec: Option<f64> }

/// Valeurs d'une niche avant notation.
#[derive(Default, Clone)]
struct Agg { islands: Vec<usize>, newcomers: Vec<usize> }

fn median(mut v: Vec<f64>) -> Option<f64> { if v.is_empty() { return None } v.sort_by(|a, b| a.partial_cmp(b).unwrap()); Some(v[v.len() / 2]) }
pub fn quantile(mut v: Vec<f64>, q: f64) -> Option<f64> { if v.is_empty() { return None } v.sort_by(|a, b| a.partial_cmp(b).unwrap()); Some(v[((v.len() - 1) as f64 * q).round() as usize]) }
/// Moyenne pondérée par les joueurs (une map à 10 joueurs ne doit pas peser autant qu'une à 10 000).
fn wmean(v: &[(f64, f64)]) -> Option<f64> { let w: f64 = v.iter().map(|x| x.1).sum(); if w <= 0.0 { None } else { Some(v.iter().map(|x| x.0 * x.1).sum::<f64>() / w) } }
/// Rang centile de x parmi vals (0..1).
pub fn pct(vals: &[f64], x: f64) -> f64 {
    if vals.len() < 2 { return 0.5 }
    let below = vals.iter().filter(|v| **v < x).count() as f64;
    let eq = vals.iter().filter(|v| **v == x).count() as f64;
    (below + eq / 2.0) / vals.len() as f64
}

/// Part des maps actives de la niche (au moins 20 joueurs) qui montent en ce moment plus vite que d'habitude.
fn rising_share(ids: &[usize], isl: &[Isl], rising: &HashSet<String>) -> Option<f64> {
    if rising.is_empty() { return None }
    let active: Vec<&Isl> = ids.iter().map(|&i| &isl[i]).filter(|it| it.best >= 20).collect();
    if active.len() < 3 { return None }
    Some(active.iter().filter(|it| rising.contains(&it.code)).count() as f64 / active.len() as f64)
}

/// Poids des 5 axes (demande, fidélité, porte ouverte, place à prendre, élan) : appris par la vérification
/// après coup quand il y a assez d'historique, sinon les poids de départ.
pub const DEFAULT_WEIGHTS: [f64; 5] = [0.22, 0.22, 0.28, 0.14, 0.14];
async fn weights(st: &AppState) -> [f64; 5] {
    let w: Option<String> = sqlx::query_scalar("SELECT weights FROM idea_model WHERE id = 1").fetch_optional(&st.db).await.ok().flatten().flatten();
    let v: Vec<f64> = w.and_then(|w| serde_json::from_str(&w).ok()).unwrap_or_default();
    if v.len() == 5 && (v.iter().sum::<f64>() - 1.0).abs() < 0.01 { [v[0], v[1], v[2], v[3], v[4]] } else { DEFAULT_WEIGHTS }
}

/// Résultat du calcul, gardé en mémoire tant que les données n'ont pas changé.
static CACHE: Mutex<Option<(String, std::sync::Arc<Value>)>> = Mutex::new(None);

/// Toutes les niches notées. Recalculées quand une nouvelle collecte a eu lieu.
pub async fn niches(st: &AppState) -> Result<std::sync::Arc<Value>, AppError> {
    let stamp: Option<String> = sqlx::query_scalar("SELECT COALESCE(last_ok, '') || '|' || COALESCE((SELECT MAX(day) FROM fn_metrics), '') || '|' || (SELECT COUNT(*) FROM tt_videos) FROM fn_status WHERE id = 1").fetch_optional(&st.db).await?;
    let stamp = stamp.unwrap_or_default();
    if let Some((s, v)) = CACHE.lock().unwrap().as_ref() { if *s == stamp { return Ok(v.clone()) } }
    let v = std::sync::Arc::new(compute(st).await?);
    *CACHE.lock().unwrap() = Some((stamp, v.clone()));
    Ok(v)
}

async fn compute(st: &AppState) -> Result<Value, AppError> {
    let rows: Vec<(String, String, String, String, i64, Option<String>)> = sqlx::query_as(
        "SELECT code, title, creator, tags, (seen_new = 1 AND first_seen > datetime('now', '-30 days')), first_seen FROM fn_islands
         WHERE code NOT LIKE 'playlist_%' AND lower(creator) <> 'epic' AND code IN (SELECT code FROM fn_metrics WHERE day >= date((SELECT MAX(day) FROM fn_metrics), '-8 days'))",
    ).fetch_all(&st.db).await?;
    let metrics: Vec<(String, String, Option<i64>, Option<f64>, Option<f64>, Option<f64>, Option<i64>)> = sqlx::query_as(
        "SELECT code, day, unique_players, avg_minutes, d1, d7, recommends FROM fn_metrics WHERE day >= date((SELECT MAX(day) FROM fn_metrics), '-7 days') ORDER BY day",
    ).fetch_all(&st.db).await?;
    let Some(last_day) = metrics.iter().filter(|m| m.2.is_some()).map(|m| m.1.clone()).max() else {
        return Ok(json!({ "ready": false, "niches": [], "islands": 0 }));
    };
    let mut days: Vec<String> = metrics.iter().map(|m| m.1.clone()).collect(); days.sort(); days.dedup();
    let first_day = days.first().cloned().unwrap_or_default();

    let mut isl: Vec<Isl> = vec![];
    let mut idx: HashMap<String, usize> = HashMap::new();
    for (code, title, _creator, tags, new, _) in &rows {
        let tags: Vec<String> = serde_json::from_str(tags).unwrap_or_default();
        idx.insert(code.clone(), isl.len());
        isl.push(Isl { code: code.clone(), title: title.clone(), text: norm(&format!("{title} {}", tags.join(" "))), is_new: *new == 1, by_day: HashMap::new(), best: 0, mins: None, d1: None, d7: None, rec: None });
    }
    for (code, day, up, mins, d1, d7, rec) in &metrics {
        let Some(&i) = idx.get(code) else { continue };
        let it = &mut isl[i];
        if let Some(u) = up { it.by_day.insert(day.clone(), *u); it.best = it.best.max(*u); if *u > 0 { it.rec = rec.map(|r| r as f64 / *u as f64) } }
        if mins.is_some() { it.mins = *mins }
        if d1.is_some() { it.d1 = *d1 }
        if d7.is_some() { it.d7 = *d7 }
    }
    let last = |it: &Isl| *it.by_day.get(&last_day).unwrap_or(&0);

    // Repères de tout l'écosystème (maps jouées le dernier jour).
    let alive: Vec<usize> = (0..isl.len()).filter(|&i| last(&isl[i]) > 0).collect();
    let eco_mins = median(alive.iter().filter_map(|&i| isl[i].mins).collect());
    let eco_d1 = median(alive.iter().filter_map(|&i| isl[i].d1).collect());

    // Thèmes connus + mots émergents.
    let mut th = themes();
    let known: HashSet<String> = THEMES.iter().flat_map(|t| t.6.split(',').flat_map(|p| p.split_whitespace().map(|w| w.to_string()).collect::<Vec<_>>())).collect();
    {
        // Mot → (îles, joueurs dernier jour, joueurs 1er jour à périmètre constant, idem dernier jour)
        let mut words: HashMap<String, (Vec<usize>, i64, i64, i64)> = HashMap::new();
        for &i in &alive {
            let mut seen = HashSet::new();
            let toks: Vec<String> = norm(&isl[i].title).split_whitespace().map(String::from).collect();
            // Mots seuls, et expressions de 2 mots (« 99 nights », « steal brainrot »…).
            let mut grams: Vec<String> = toks.iter().filter(|w| w.len() >= 4 && !w.chars().all(|c| c.is_ascii_digit()) && !STOP.contains(&w.as_str()) && !known.contains(w.as_str())).cloned().collect();
            for p in toks.windows(2) {
                let ok = |w: &String| w.len() >= 2 && !STOP.contains(&w.as_str());
                if ok(&p[0]) && ok(&p[1]) && !(known.contains(&p[0]) && known.contains(&p[1])) { grams.push(format!("{} {}", p[0], p[1])) }
            }
            for w in grams {
                if !seen.insert(w.clone()) { continue }
                let e = words.entry(w).or_default();
                e.0.push(i); e.1 += last(&isl[i]);
                if let (Some(f), Some(l)) = (isl[i].by_day.get(&first_day), isl[i].by_day.get(&last_day)) { e.2 += f; e.3 += l }
            }
        }
        let mut cand: Vec<(String, f64, i64)> = words.into_iter().filter_map(|(w, (is, p, f, l))| {
            let creators: HashSet<&str> = is.iter().map(|&i| isl[i].code.as_str()).collect();
            let trend = if f > 0 { l as f64 / f as f64 - 1.0 } else { 0.0 };
            (is.len() >= 4 && creators.len() >= 4 && p >= 1000 && trend >= 0.2).then_some((w, trend, p))
        }).collect();
        cand.sort_by(|a, b| (b.1 * (b.2 as f64).ln()).partial_cmp(&(a.1 * (a.2 as f64).ln())).unwrap());
        // Une expression couvre ses mots : on ne garde pas « nights » si « 99 nights » est déjà retenu avec les mêmes maps.
        let mut kept: Vec<String> = vec![];
        let cand: Vec<(String, f64, i64)> = cand.into_iter().filter(|(w, _, _)| {
            if kept.iter().any(|k| k.contains(w.as_str()) || w.contains(k.as_str())) { return false }
            kept.push(w.clone()); true
        }).collect();
        for (w, _, _) in cand.into_iter().take(10) {
            let label = { let mut c = w.chars(); c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default() };
            th.push(Theme { key: format!("w:{w}"), fr: label.clone(), en: label, genre: "", complexity: 2, skills: vec!["verse", "build"], flag: "", pats: vec![format!(" {w} ")], emerging: true });
        }
    }

    // Îles de chaque niche.
    let mut aggs: Vec<Agg> = vec![Agg::default(); th.len()];
    let mut themes_of: Vec<Vec<usize>> = vec![vec![]; isl.len()];
    let risky: Vec<usize> = th.iter().enumerate().filter(|(_, t)| t.flag == "risque").map(|(i, _)| i).collect();
    for (i, it) in isl.iter().enumerate() {
        // Les maps XP / AFK faussent les chiffres des niches qu'elles imitent (temps de jeu gonflé) : elles ne comptent que dans leur propre niche.
        let is_risky = risky.iter().any(|&r| th[r].pats.iter().any(|p| it.text.contains(p.as_str())));
        for (t, theme) in th.iter().enumerate() {
            if is_risky && theme.flag != "risque" { continue }
            if theme.pats.iter().any(|p| it.text.contains(p.as_str())) {
                themes_of[i].push(t);
                if last(it) > 0 { aggs[t].islands.push(i) }
                if it.is_new { aggs[t].newcomers.push(i) }
            }
        }
    }
    // Genre d'une niche émergente : le genre le plus fréquent de ses îles.
    for t in 0..th.len() {
        if !th[t].genre.is_empty() { continue }
        let mut c: HashMap<&'static str, usize> = HashMap::new();
        for &i in &aggs[t].islands { for &o in &themes_of[i] { if !th[o].genre.is_empty() { *c.entry(th[o].genre).or_default() += 1 } } }
        th[t].genre = c.into_iter().max_by_key(|x| x.1).map(|x| x.0).unwrap_or("fete");
    }

    // Vidéos récentes (tous les studios) : mentions de chaque niche sur 7 jours et sur les 7 jours d'avant.
    let vids: Vec<(String, String, Option<String>, String)> = sqlx::query_as("SELECT text, hashtags, created, fetched_at FROM tt_videos WHERE fetched_at > datetime('now', '-21 days')").fetch_all(&st.db).await?;
    let today = chrono::Utc::now().date_naive();
    let d7 = (today - chrono::Duration::days(7)).to_string();
    let d14 = (today - chrono::Duration::days(14)).to_string();
    let vtexts: Vec<(String, bool, bool)> = vids.iter().map(|(t, h, c, f)| {
        let day = c.as_deref().unwrap_or(f).chars().take(10).collect::<String>();
        (norm(&format!("{t} {}", h.replace(['[', ']', '"', '#', ','], " "))), day >= d7, day >= d14 && day < d7)
    }).collect();

    // Historique des niches (au-delà des 7 jours de l'API).
    let hist: Vec<(String, String, i64)> = sqlx::query_as("SELECT niche, day, players FROM fn_niches WHERE day >= date(?, '-15 days')").bind(&last_day).fetch_all(&st.db).await?;
    let ago14 = chrono::NaiveDate::parse_from_str(&last_day, "%Y-%m-%d").map(|d| (d - chrono::Duration::days(14)).to_string()).unwrap_or_default();

    struct N { t: usize, players: i64, typical: f64, trend: Option<f64>, trend14: Option<f64>, mins: Option<f64>, d1: Option<f64>, d7: Option<f64>, rec: Option<f64>,
        new_n: usize, s100: usize, s1000: usize, top3: f64, lead_q: Option<f64>, v7: usize, vprev: usize }
    let mut ns: Vec<N> = vec![];
    for (t, a) in aggs.iter().enumerate() {
        if a.islands.len() < 3 { continue }
        let players: i64 = a.islands.iter().map(|&i| last(&isl[i])).sum();
        if players <= 0 { continue }
        let mut ups: Vec<i64> = a.islands.iter().map(|&i| last(&isl[i])).collect(); ups.sort();
        let typical = ups[ups.len() / 2] as f64;
        let trend = trend_of(&a.islands, &isl, &days);
        let old = hist.iter().filter(|h| h.0 == th[t].key && h.1 <= ago14).max_by(|a, b| a.1.cmp(&b.1)).map(|h| h.2);
        let trend14 = old.filter(|o| *o > 0).map(|o| players as f64 / o as f64 - 1.0);
        let w = |g: fn(&Isl) -> Option<f64>| wmean(&a.islands.iter().filter_map(|&i| g(&isl[i]).map(|v| (v, last(&isl[i]) as f64))).collect::<Vec<_>>());
        let mut by_players = a.islands.clone(); by_players.sort_by_key(|&i| -last(&isl[i]));
        let top3 = by_players.iter().take(3).map(|&i| last(&isl[i])).sum::<i64>() as f64 / players as f64;
        // Qualité des 5 premières maps, rapportée à l'écosystème (1 = comme une map typique).
        let lead: Vec<f64> = by_players.iter().take(5).filter_map(|&i| {
            let m = isl[i].mins.zip(eco_mins).map(|(m, e)| if e > 0.0 { m / e } else { 1.0 });
            let d = isl[i].d1.zip(eco_d1).map(|(d, e)| if e > 0.0 { d / e } else { 1.0 });
            match (m, d) { (Some(m), Some(d)) => Some((m + d) / 2.0), (Some(x), None) | (None, Some(x)) => Some(x), _ => None }
        }).collect();
        let lead_q = if lead.is_empty() { None } else { Some(lead.iter().sum::<f64>() / lead.len() as f64) };
        let (mut v7, mut vprev) = (0, 0);
        for (txt, recent, prev) in &vtexts { if th[t].pats.iter().any(|p| txt.contains(p.as_str())) { if *recent { v7 += 1 } else if *prev { vprev += 1 } } }
        ns.push(N { t, players, typical, trend, trend14, mins: w(|x| x.mins), d1: w(|x| x.d1), d7: w(|x| x.d7), rec: w(|x| x.rec),
            new_n: a.newcomers.len(), s100: a.newcomers.iter().filter(|&&i| isl[i].best >= 100).count(), s1000: a.newcomers.iter().filter(|&&i| isl[i].best >= 1000).count(),
            top3, lead_q, v7, vprev });
    }

    // Garde l'historique du jour.
    for n in &ns {
        let _ = sqlx::query("INSERT INTO fn_niches (day, niche, players, islands) VALUES (?, ?, ?, ?) ON CONFLICT(day, niche) DO UPDATE SET players = excluded.players, islands = excluded.islands")
            .bind(&last_day).bind(&th[n.t].key).bind(n.players).bind(aggs[n.t].islands.len() as i64).execute(&st.db).await;
    }

    // Notation en centiles.
    let col = |f: &dyn Fn(&N) -> Option<f64>| ns.iter().filter_map(|n| f(n)).collect::<Vec<f64>>();
    let wt = weights(st).await;
    let rising: HashSet<String> = crate::analytics::rising_codes(st).await;
    let c_players = col(&|n| Some((n.players as f64).ln_1p()));
    let c_typ = col(&|n| Some(n.typical.ln_1p()));
    let c_mins = col(&|n| n.mins); let c_d1 = col(&|n| n.d1); let c_d7 = col(&|n| n.d7); let c_rec = col(&|n| n.rec);
    // Taux de réussite lissés : avec peu de nouvelles maps, on reste proche de la moyenne de toutes les niches.
    let (tn, t100, t1000) = ns.iter().fold((0usize, 0usize, 0usize), |a, n| (a.0 + n.new_n, a.1 + n.s100, a.2 + n.s1000));
    let p100 = if tn > 0 { t100 as f64 / tn as f64 } else { 0.3 };
    let p1000 = if tn > 0 { t1000 as f64 / tn as f64 } else { 0.05 };
    const K: f64 = 6.0;
    let r100 = |n: &N| (n.s100 as f64 + p100 * K) / (n.new_n as f64 + K);
    let r1000 = |n: &N| (n.s1000 as f64 + p1000 * K) / (n.new_n as f64 + K);
    let c_s100 = col(&|n| Some(r100(n)));
    let c_s1000 = col(&|n| Some(r1000(n)));
    let c_trend = col(&|n| n.trend);
    let c_vid = col(&|n| (n.v7 + n.vprev >= 3).then(|| (n.v7 as f64 + 1.0) / (n.vprev as f64 + 1.0)));
    let c_lead = col(&|n| n.lead_q);
    let c_count = ns.iter().map(|n| aggs[n.t].islands.len() as f64).collect::<Vec<f64>>();
    let avg = |v: Vec<Option<f64>>| { let v: Vec<f64> = v.into_iter().flatten().collect(); if v.is_empty() { 0.5 } else { v.iter().sum::<f64>() / v.len() as f64 } };
    let month = chrono::Utc::now().format("%m").to_string().parse::<u32>().unwrap_or(1);
    let season_now = |flag: &str| match flag { "noel" => [11, 12, 1].contains(&month), "halloween" => [9, 10].contains(&month), "ete" => [5, 6, 7, 8].contains(&month), _ => false };
    let med_islands = median(ns.iter().map(|n| aggs[n.t].islands.len() as f64).collect()).unwrap_or(0.0);
    let med_typ = median(ns.iter().map(|n| n.typical).collect()).unwrap_or(0.0);

    let mut scored: Vec<(f64, Value)> = vec![];
    let mut opp_by_t: HashMap<usize, f64> = HashMap::new();
    for n in &ns {
        let theme = &th[n.t];
        let demand = avg(vec![Some(pct(&c_players, (n.players as f64).ln_1p())), Some(pct(&c_typ, n.typical.ln_1p()))]);
        let fidelity = avg(vec![n.mins.map(|x| pct(&c_mins, x)), n.d1.map(|x| pct(&c_d1, x)), n.d7.map(|x| pct(&c_d7, x)), n.rec.map(|x| pct(&c_rec, x))]);
        let open = 0.5 * pct(&c_s100, r100(n)) + 0.2 * pct(&c_s1000, r1000(n)) + 0.3 * (1.0 - n.top3);
        let lead_pct = n.lead_q.map(|q| pct(&c_lead, q)).unwrap_or(0.5);
        let gap = demand * (1.0 - lead_pct);
        let vid = (n.v7 + n.vprev >= 3).then(|| pct(&c_vid, (n.v7 as f64 + 1.0) / (n.vprev as f64 + 1.0)));
        let hourly = rising_share(&aggs[n.t].islands, &isl, &rising);
        let momentum = avg(vec![n.trend.map(|x| pct(&c_trend, x)), vid, n.trend14.map(|x| (0.5 + x).clamp(0.0, 1.0)), hourly.map(|h| (0.5 + h * 2.0).min(1.0))]);
        let mut score = 100.0 * (wt[0] * demand + wt[1] * fidelity + wt[2] * open + wt[3] * gap + wt[4] * momentum);
        // Vague qui naît : ça monte fort et il y a encore peu de maps. C'est là qu'on arrive avant les autres.
        let crowd = pct(&c_count, aggs[n.t].islands.len() as f64);
        let wave = momentum >= 0.7 && crowd <= 0.5;
        if wave { score += 15.0 * momentum * (1.0 - crowd) }
        let dominated = n.top3 >= 0.8 && aggs[n.t].islands.len() >= 10;
        if dominated { score *= 0.85 }
        let score = score.min(100.0);
        let score = if season_now(theme.flag) { (score * 1.1).min(100.0) } else { score };
        let confidence = if aggs[n.t].islands.len() >= 8 && n.new_n >= 3 { "bonne" } else if aggs[n.t].islands.len() >= 5 { "moyenne" } else { "faible" };

        let mut badges: Vec<&str> = vec![];
        if n.new_n >= 3 && n.s100 as f64 / n.new_n as f64 >= 0.4 { badges.push("porte_ouverte") }
        if n.trend.unwrap_or(0.0) >= 0.15 || n.trend14.unwrap_or(0.0) >= 0.3 { badges.push("ca_monte") }
        if demand >= 0.6 && lead_pct <= 0.4 { badges.push("joueurs_decus") }
        if n.v7 >= 5 && n.v7 as f64 >= (n.vprev as f64 + 1.0) * 1.3 { badges.push("tendance_video") }
        if (aggs[n.t].islands.len() as f64) <= med_islands * 0.5 && n.typical >= med_typ { badges.push("peu_de_concurrence") }
        if fidelity >= 0.7 { badges.push("joueurs_fideles") }
        if theme.emerging { badges.push("nouveau_mot") }
        if season_now(theme.flag) { badges.push("saison") }
        if wave { badges.push("vague") }
        if dominated { badges.push("domine") }

        let mut lead = aggs[n.t].islands.clone(); lead.sort_by_key(|&i| -last(&isl[i]));
        let top10: Vec<usize> = lead.iter().take(10).copied().collect();
        let study: Vec<Value> = lead.iter().take(3).map(|&i| json!({ "code": isl[i].code, "title": isl[i].title, "players": last(&isl[i]), "minutes": isl[i].mins.map(|m| m.round()), "d1": isl[i].d1 })).collect();
        let best_new = aggs[n.t].newcomers.iter().max_by_key(|&&i| isl[i].best).filter(|&&i| isl[i].best > 0)
            .map(|&i| json!({ "code": isl[i].code, "title": isl[i].title, "players": isl[i].best, "minutes": isl[i].mins.map(|m| m.round()), "d1": isl[i].d1 }));
        // Mots les plus repris dans les titres de la niche (à éviter pour ne pas ressembler aux autres).
        let mut wc: HashMap<String, usize> = HashMap::new();
        for &i in &aggs[n.t].islands { let mut s = HashSet::new(); for w in norm(&isl[i].title).split_whitespace() { if w.len() >= 3 && !STOP.contains(&w) && s.insert(w.to_string()) { *wc.entry(w.to_string()).or_default() += 1 } } }
        let mut wv: Vec<(String, usize)> = wc.into_iter().filter(|x| x.1 >= 3).collect(); wv.sort_by(|a, b| b.1.cmp(&a.1));
        let series: Vec<Value> = days.iter().map(|d| json!({ "day": d, "players": aggs[n.t].islands.iter().filter_map(|&i| isl[i].by_day.get(d)).sum::<i64>() })).collect();

        opp_by_t.insert(n.t, score);
        let _ = sqlx::query("UPDATE fn_niches SET score = ?, axes = ? WHERE day = ? AND niche = ?")
            .bind(score).bind(json!([demand, fidelity, open, gap, momentum]).to_string()).bind(&last_day).bind(&theme.key).execute(&st.db).await;
        scored.push((score, json!({
            "key": theme.key, "generated_day": last_day, "fr": theme.fr, "en": theme.en, "genre": theme.genre, "complexity": theme.complexity, "skills": theme.skills,
            "emerging": theme.emerging, "season": season_now(theme.flag), "risky": theme.flag == "risque",
            "score": score.round(), "confidence": confidence, "badges": badges,
            "axes": { "demand": (demand * 100.0).round(), "fidelity": (fidelity * 100.0).round(), "open": (open * 100.0).round(), "gap": (gap * 100.0).round(), "momentum": (momentum * 100.0).round() },
            "islands": aggs[n.t].islands.len(), "players": n.players, "typical": n.typical.round(),
            "trend": n.trend.map(|x| (x * 100.0).round()), "trend14": n.trend14.map(|x| (x * 100.0).round()),
            "minutes": n.mins.map(|x| x.round()), "d1": n.d1, "d7": n.d7, "rec_rate": n.rec,
            "eco": { "minutes": eco_mins.map(|x| x.round()), "d1": eco_d1 },
            "newcomers": n.new_n, "success100": n.s100, "success1000": n.s1000, "top3_share": (n.top3 * 100.0).round(),
            "videos": n.v7, "videos_prev": n.vprev,
            "targets": { "minutes": quantile(top10.iter().filter_map(|&i| isl[i].mins).collect(), 0.75).map(|x| x.round()), "d1": quantile(top10.iter().filter_map(|&i| isl[i].d1).collect(), 0.75) },
            "study": study, "best_new": best_new, "title_words": wv.into_iter().take(6).map(|x| x.0).collect::<Vec<_>>(), "series": series,
        })));
    }
    // Twists : croiser la niche avec une autre niche forte que presque personne ne combine encore.
    let tset: Vec<HashSet<usize>> = aggs.iter().map(|a| a.islands.iter().copied().collect()).collect();
    for (_, v) in scored.iter_mut() {
        let key = v["key"].as_str().unwrap_or("").to_string();
        let Some(t) = th.iter().position(|x| x.key == key) else { continue };
        let mut tw: Vec<(f64, Value)> = opp_by_t.iter().filter(|(&o, _)| o != t && th[o].flag != "risque" && (th[o].genre != th[t].genre || th[o].emerging)).map(|(&o, &s)| {
            let both = tset[t].intersection(&tset[o]).count();
            (s / (1.0 + both as f64), json!({ "key": th[o].key, "fr": th[o].fr, "en": th[o].en, "maps_together": both, "score": s.round() }))
        }).collect();
        tw.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        v["twists"] = json!(tw.into_iter().take(2).map(|x| x.1).collect::<Vec<_>>());
    }
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    Ok(json!({ "ready": true, "last_day": last_day, "first_day": first_day, "islands": alive.len(), "niches": scored.into_iter().map(|x| x.1).collect::<Vec<_>>() }))
}

// ---------- Profil et idées de chaque créateur ----------

/// Réglages du créateur : seulement les alertes (les idées sont les mêmes pour tous).
async fn settings(st: &AppState, uid: i64) -> Result<(bool, Option<String>), AppError> {
    let r: Option<(i64, Option<String>)> = sqlx::query_as("SELECT notify, generated_at FROM idea_profiles WHERE user_id = ?").bind(uid).fetch_optional(&st.db).await?;
    Ok(r.map(|r| (r.0 == 1, r.1)).unwrap_or((true, None)))
}

/// Recalcule les idées d'un créateur (au plus toutes les 3 heures, sauf `force`). Renvoie les nouvelles idées.
pub async fn refresh_user(st: &AppState, uid: i64, force: bool) -> Result<Vec<(i64, String)>, AppError> {
    let (notify, gen) = settings(st, uid).await?;
    if !force {
        if let Some(g) = gen { let stale: bool = sqlx::query_scalar("SELECT ? < datetime('now', '-3 hours')").bind(&g).fetch_one(&st.db).await?; if !stale { return Ok(vec![]) } }
    }
    let all = niches(st).await?;
    let list = all["niches"].as_array().cloned().unwrap_or_default();
    if list.is_empty() { return Ok(vec![]) }
    // Les meilleures opportunités du marché (hors maps XP et niches aux données trop minces).
    let top: Vec<Value> = list.into_iter().filter(|n| !n["risky"].as_bool().unwrap_or(false) && n["confidence"] != "faible").take(12).collect();
    sqlx::query("UPDATE ideas SET active = 0 WHERE user_id = ?").bind(uid).execute(&st.db).await?;
    let mut fresh = vec![];
    for n in &top {
        let (rank, f) = (n["score"].as_f64().unwrap_or(0.0), 0.0);
        let key = n["key"].as_str().unwrap_or("");
        let existing: Option<i64> = sqlx::query_scalar("SELECT id FROM ideas WHERE user_id = ? AND niche = ?").bind(uid).bind(key).fetch_optional(&st.db).await?;
        match existing {
            Some(id) => { sqlx::query("UPDATE ideas SET score = ?, fit = ?, rank = ?, data = ?, active = 1, updated_at = datetime('now') WHERE id = ?").bind(n["score"].as_f64()).bind(f).bind(rank).bind(n.to_string()).bind(id).execute(&st.db).await?; }
            None => {
                let id: i64 = sqlx::query_scalar("INSERT INTO ideas (user_id, niche, score, fit, rank, data) VALUES (?, ?, ?, ?, ?, ?) RETURNING id").bind(uid).bind(key).bind(n["score"].as_f64()).bind(f).bind(rank).bind(n.to_string()).fetch_one(&st.db).await?;
                fresh.push((id, n["fr"].as_str().unwrap_or(key).to_string()));
            }
        }
    }
    sqlx::query("INSERT INTO idea_profiles (user_id, generated_at) VALUES (?, datetime('now')) ON CONFLICT(user_id) DO UPDATE SET generated_at = datetime('now')").bind(uid).execute(&st.db).await?;
    // Alertes, comme une nouvelle annonce : au plus 3 à la fois, seulement si le créateur les veut.
    if notify && gen_is_some(st, uid).await {
        for (id, label) in fresh.iter().take(3) {
            let _ = sqlx::query("INSERT INTO notifications (user_id, map_id, text, link) VALUES (?, NULL, ?, ?)")
                .bind(uid).bind(format!("Nouvelle idée de map : {label}")).bind(format!("/analyse/idees?i={id}")).execute(&st.db).await;
        }
    }
    Ok(fresh)
}

/// Pas d'alerte au tout premier calcul (sinon 12 notifications d'un coup).
async fn gen_is_some(st: &AppState, uid: i64) -> bool {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM ideas WHERE user_id = ? AND created_at < datetime('now', '-1 minute')").bind(uid).fetch_one(&st.db).await.unwrap_or(0) > 0
}

/// Après chaque collecte : idées des créateurs qui ont le plan et les alertes activées.
pub async fn refresh_all(st: &AppState) {
    let users: Vec<i64> = sqlx::query_scalar("SELECT u.id FROM users u LEFT JOIN idea_profiles p ON p.user_id = u.id WHERE u.disabled = 0 AND ((u.plan = 'pro' AND (u.plan_source <> 'stripe' OR u.sub_status IN ('active', 'trialing', 'past_due'))) OR u.trial_until > datetime('now')) AND COALESCE(p.notify, 1) = 1 AND u.last_seen > datetime('now', '-30 days')")
        .fetch_all(&st.db).await.unwrap_or_default();
    for uid in users { if let Err(e) = refresh_user(st, uid, false).await { tracing::warn!("idées ({uid}) : {e:?}") } }
}

// ---------- API ----------

pub async fn list(State(st): State<AppState>, u: AuthUser) -> ApiResult<Value> {
    crate::billing::require_analyse(&st, &u).await?;
    refresh_user(&st, u.id, false).await?;
    let (notify, gen) = settings(&st, u.id).await?;
    let rows: Vec<(i64, String, f64, f64, String, String, Option<i64>, String, i64)> = sqlx::query_as(
        "SELECT id, niche, score, fit, data, status, map_id, created_at, active FROM ideas WHERE user_id = ? AND status <> 'dismissed' AND (active = 1 OR status IN ('saved', 'started')) ORDER BY active DESC, rank DESC").bind(u.id).fetch_all(&st.db).await?;
    let all = niches(&st).await?;
    Ok(Json(json!({
        "notify": notify,
        "generated_at": gen, "ready": all["ready"], "last_day": all["last_day"], "islands": all["islands"], "niches_count": all["niches"].as_array().map(|a| a.len()).unwrap_or(0),
        "model": crate::model::status(&st).await,
        "ideas": rows.into_iter().map(|r| { let mut d: Value = serde_json::from_str(&r.4).unwrap_or(json!({})); d["id"] = json!(r.0); d["status"] = json!(r.5); d["map_id"] = json!(r.6); d["created_at"] = json!(r.7); d["active"] = json!(r.8 == 1); d }).collect::<Vec<_>>(),
    })))
}

/// Toutes les niches notées (pour comparer et creuser).
pub async fn all_niches(State(st): State<AppState>, u: AuthUser) -> ApiResult<Value> {
    crate::billing::require_analyse(&st, &u).await?;
    Ok(Json((*niches(&st).await?).clone()))
}

#[derive(Deserialize)]
pub struct SettingsIn { notify: bool }

/// Alertes de nouvelles idées : oui ou non.
pub async fn save_settings(State(st): State<AppState>, u: AuthUser, Json(b): Json<SettingsIn>) -> ApiResult<Value> {
    crate::billing::require_analyse(&st, &u).await?;
    sqlx::query("INSERT INTO idea_profiles (user_id, notify) VALUES (?, ?) ON CONFLICT(user_id) DO UPDATE SET notify = excluded.notify, updated_at = datetime('now')")
        .bind(u.id).bind(b.notify as i64).execute(&st.db).await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct StateIn { status: String }

pub async fn set_state(State(st): State<AppState>, u: AuthUser, Path(id): Path<i64>, Json(b): Json<StateIn>) -> ApiResult<Value> {
    crate::billing::require_analyse(&st, &u).await?;
    if !["new", "seen", "saved", "dismissed"].contains(&b.status.as_str()) { return Err(bad("État inconnu.")) }
    let n = sqlx::query("UPDATE ideas SET status = ? WHERE id = ? AND user_id = ? AND status <> 'started'").bind(&b.status).bind(id).bind(u.id).execute(&st.db).await?.rows_affected();
    if n == 0 { return Err(AppError::NotFound) }
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct StartIn { studio_id: i64, name: Option<String> }

/// « Je me lance » : crée la map avec la fiche de l'idée en page.
pub async fn start(State(st): State<AppState>, u: AuthUser, Path(id): Path<i64>, Json(b): Json<StartIn>) -> ApiResult<Value> {
    crate::billing::require_analyse(&st, &u).await?;
    if crate::access::studio_role(&st, &u, b.studio_id).await? < crate::access::Role::Admin { return Err(AppError::Forbidden) }
    let (data,): (String,) = sqlx::query_as("SELECT data FROM ideas WHERE id = ? AND user_id = ?").bind(id).bind(u.id).fetch_optional(&st.db).await?.ok_or(AppError::NotFound)?;
    let d: Value = serde_json::from_str(&data).unwrap_or(json!({}));
    let label = d["fr"].as_str().unwrap_or("Nouvelle map").to_string();
    let name: String = b.name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).unwrap_or_else(|| label.clone()).chars().take(60).collect();
    let mid = crate::studio::insert_map(&st, b.studio_id, &name, d["genre"].as_str().unwrap_or(""), None).await?;
    let blocks = crate::pages::idea_blocks(&d);
    let title = format!("Fiche idée : {label}");
    let content = blocks.to_string();
    let pid: i64 = sqlx::query_scalar("INSERT INTO pages (map_id, title, icon, blocks, created_by, updated_by) VALUES (?, ?, 'I', ?, ?, ?) RETURNING id")
        .bind(mid).bind(&title).bind(&content).bind(u.id).bind(u.id).fetch_one(&st.db).await?;
    crate::pages::snapshot(&st, pid, 1, &title, &content, u.id).await?;
    sqlx::query("UPDATE ideas SET status = 'started', map_id = ? WHERE id = ?").bind(mid).bind(id).execute(&st.db).await?;
    Ok(Json(json!({ "map_id": mid, "page_id": pid })))
}
