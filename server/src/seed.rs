//! Données d'exemple : P0P Studio, map VaultRush (gendarmes et voleurs tycoon 8v8), 4 membres.
use crate::{auth::hash_password, studio::insert_map, AppState};
use serde_json::json;
use time::{macros::format_description, Duration, OffsetDateTime};

pub const DEMO_PASSWORD: &str = "demo-pop-2026";

fn day(offset: i64) -> String {
    (OffsetDateTime::now_utc().date() + Duration::days(offset)).format(format_description!("[year]-[month]-[day]")).unwrap()
}
fn month(offset: i32) -> String {
    let d = OffsetDateTime::now_utc().date();
    let (mut y, mut m) = (d.year(), d.month() as i32 + offset);
    while m < 1 {
        m += 12;
        y -= 1;
    }
    format!("{y:04}-{m:02}")
}
fn id() -> String {
    crate::auth::random_token()[..12].to_string()
}

pub async fn run(st: &AppState) -> anyhow::Result<()> {
    let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE email = 'yanis@demo.pop'").fetch_one(&st.db).await?;
    if exists > 0 {
        println!("Données d'exemple déjà présentes.");
        return Ok(());
    }
    let hash = hash_password(DEMO_PASSWORD).map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let mut uid = std::collections::HashMap::new();
    for (key, email, name) in [
        ("Y", "yanis@demo.pop", "Yanis Belkacem"), ("C", "chloe@demo.pop", "Chloé Martin"), ("H", "hugo@demo.pop", "Hugo Lefèvre"), ("I", "ines@demo.pop", "Inès Diallo"),
        ("K", "kiro@demo.pop", "Kiro"), ("P", "pixelmoon@demo.pop", "Pixelmoon"), ("S", "nord@demo.pop", "Studio Nord"),
    ] {
        let i: i64 = sqlx::query_scalar("INSERT INTO users (email, name, password_hash, credits) VALUES (?, ?, ?, 120) RETURNING id").bind(email).bind(name).bind(&hash).fetch_one(&st.db).await?;
        uid.insert(key, i);
    }
    let sid: i64 = sqlx::query_scalar("INSERT INTO studios (name) VALUES ('P0P Studio') RETURNING id").fetch_one(&st.db).await?;
    for (k, role, spec, folio, h) in [
        ("Y", "owner", "Code Verse", "3 maps publiées, dont un tycoon à 2 000 joueurs simultanés", 20),
        ("C", "admin", "Carte", "Level design de 5 îles compétitives", 15),
        ("H", "member", "Conception", "Économie et interface de 2 tycoons", 10),
        ("I", "freelance", "Test", "QA pour 12 maps, rapports sous 24 h", 8),
    ] {
        sqlx::query("INSERT INTO memberships (studio_id, user_id, role, specialty, portfolio, hours_per_week) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(sid).bind(uid[k]).bind(role).bind(spec).bind(folio).bind(h).execute(&st.db).await?;
    }
    let mid = insert_map(st, sid, "VaultRush", "Gendarmes et voleurs tycoon 8v8", Some(&day(9))).await?;
    sqlx::query("UPDATE maps SET status = 'early_access' WHERE id = ?").bind(mid).execute(&st.db).await?;
    insert_map(st, sid, "Prison Pop", "Évasion en coopération, 4 joueurs", None).await?;

    // 30 tâches : (n°, titre, étiquette, membre, statut, jour prévu, dépendances)
    let tasks: Vec<(i64, &str, &str, &str, &str, i64, Vec<i64>)> = vec![
        (1, "Créer le projet UEFN et le dépôt Lore", "code", "Y", "done", -40, vec![]),
        (2, "Blocking de la ville et de la banque", "carte", "C", "done", -38, vec![]),
        (3, "Équipes gendarmes / voleurs 8v8", "code", "Y", "done", -36, vec![1]),
        (4, "Boucle de jeu (1 page)", "conception", "H", "done", -40, vec![]),
        (5, "Coffre-fort et minuteur de braquage", "code", "Y", "done", -30, vec![3]),
        (6, "Habillage du commissariat", "carte", "C", "done", -20, vec![2]),
        (7, "Tableau des prix et des dépenses", "conception", "H", "done", -25, vec![4]),
        (8, "Sauvegarde persistante", "code", "Y", "done", -6, vec![3]),
        (9, "Playtest interne 4v4", "test", "I", "done", -7, vec![5]),
        (10, "Prison et système d’arrestation", "code", "Y", "done", -2, vec![3]),
        (14, "Compteur de butin dans le HUD", "code", "Y", "doing", 0, vec![]),
        (15, "Collision des toits de la banque", "carte", "C", "doing", 0, vec![]),
        (16, "Textes du tutoriel", "conception", "H", "doing", 0, vec![]),
        (17, "Test de régression sauvegarde", "test", "I", "doing", 0, vec![8]),
        (18, "Véhicules de police (spawners)", "carte", "C", "doing", -1, vec![]),
        (19, "Boutique d’améliorations des voleurs", "code", "Y", "doing", 0, vec![7]),
        (20, "Vignette et description", "conception", "H", "doing", 1, vec![]),
        (21, "Hélicoptère d’évasion", "code", "Y", "blocked", 2, vec![18]),
        (22, "Équilibrage du butin 8v8", "conception", "H", "blocked", 2, vec![19]),
        (23, "Playtest 8v8 (16 joueurs)", "test", "I", "blocked", 4, vec![21]),
        (24, "Optimisation mémoire de l’île", "carte", "C", "blocked", 3, vec![15]),
        (25, "Offre en jeu « Pack Braqueur »", "code", "Y", "todo", 2, vec![19]),
        (26, "Caméras de surveillance", "carte", "C", "todo", 1, vec![]),
        (27, "Notes de patch v1.0", "conception", "H", "todo", 6, vec![]),
        (28, "Checklist de soumission Epic", "test", "I", "todo", 5, vec![20]),
        (29, "Questionnaire du playtest 8v8", "test", "I", "todo", 3, vec![]),
        (30, "Sons d’alarme de la banque", "carte", "C", "todo", 2, vec![]),
        (31, "Classement de fin de manche", "code", "Y", "todo", 1, vec![]),
        (32, "Traduction FR / EN", "conception", "H", "todo", 5, vec![]),
        (33, "Trailer de 20 s", "conception", "H", "todo", 6, vec![20]),
    ];
    let mut tid = std::collections::HashMap::new();
    for (n, title, tag, who, status, d, _) in &tasks {
        let done_at = if *status == "done" { Some(format!("{} 18:00:00", day(*d))) } else { None };
        let i: i64 = sqlx::query_scalar("INSERT INTO tasks (map_id, number, title, tag, status, assignee_id, planned_day, done_at, lot) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id")
            .bind(mid).bind(n).bind(title).bind(tag).bind(status).bind(uid[who]).bind(day(*d)).bind(done_at)
            .bind(if *n <= 4 { "Lot 1 · Fondations" } else if *n <= 18 || *n == 21 { "Lot 2 · Braquage" } else if [7, 19, 22, 25].contains(n) { "Lot 3 · Économie" } else { "Lot 4 · Tests et soumission" })
            .fetch_one(&st.db).await?;
        tid.insert(*n, i);
        sqlx::query("INSERT INTO task_events (task_id, user_id, what, via, at) VALUES (?, ?, 'Tâche créée', 'Générateur de projet', datetime('now', '-45 days'))").bind(i).bind(uid["Y"]).execute(&st.db).await?;
    }
    for (n, _, _, _, _, _, deps) in &tasks {
        for d in deps {
            sqlx::query("INSERT INTO task_deps (task_id, depends_on) VALUES (?, ?)").bind(tid[n]).bind(tid[d]).execute(&st.db).await?;
        }
    }
    sqlx::query("UPDATE tasks SET description = ?, origin = 'board:1:s3' WHERE id = ?")
        .bind("L’hélicoptère se pose sur le toit de la banque après 70 % du butin volé. Les voleurs qui montent à bord gagnent la manche.")
        .bind(tid[&21]).execute(&st.db).await?;
    for (t, done) in [("Modèle et point d’atterrissage", 1), ("Déclencheur à 70 % du butin", 1), ("Synchroniser avec les véhicules de police (#18)", 0), ("Tester en 8v8", 0)] {
        sqlx::query("INSERT INTO subtasks (task_id, title, done) VALUES (?, ?, ?)").bind(tid[&21]).bind(t).bind(done).execute(&st.db).await?;
    }
    for (who, what, via, ago) in [("Y", "À faire → En cours", "Discord · /tache-suivante", "-3 days"), ("Y", "Check-in : escape_heli.verse", "Lore / UEFN", "-2 days"), ("Y", "En cours → Bloqué", "Bouton", "-1 days")] {
        sqlx::query("INSERT INTO task_events (task_id, user_id, what, via, at) VALUES (?, ?, ?, ?, datetime('now', ?))").bind(tid[&21]).bind(uid[who]).bind(what).bind(via).bind(ago).execute(&st.db).await?;
    }
    for (n, who, body) in [(21, "H", "Le prix de l’hélicoptère est dans la page Économie."), (21, "Y", "J’attends les véhicules de Chloé pour caler le point de départ."), (18, "C", "Encore 2 spawners à placer, demain matin.")] {
        sqlx::query("INSERT INTO comments (map_id, target, user_id, body) VALUES (?, ?, ?, ?)").bind(mid).bind(format!("task:{}", tid[&n])).bind(uid[who]).bind(body).execute(&st.db).await?;
    }
    for (path, who, n, ago) in [("Content/Maps/VaultRush_City.umap", "C", 15, "-45 minutes"), ("Verse/escape_heli.verse", "Y", 21, "-26 hours"), ("Verse/hud_loot.verse", "Y", 14, "-2 hours"), ("Content/Vehicles/BP_PoliceCar.uasset", "C", 18, "-3 hours")] {
        sqlx::query("INSERT INTO file_locks (map_id, path, user_id, task_number, since) VALUES (?, ?, ?, ?, datetime('now', ?))").bind(mid).bind(path).bind(uid[who]).bind(n).bind(ago).execute(&st.db).await?;
    }

    for (t, d, k) in [("Gel du contenu", 2, "gel"), ("Playtest 8v8", 4, "test"), ("Soumission à Epic", 5, "soumission"), ("Sortie v1.0", 9, "sortie")] {
        sqlx::query("INSERT INTO milestones (map_id, title, date, kind) VALUES (?, ?, ?, ?)").bind(mid).bind(t).bind(day(d)).bind(k).execute(&st.db).await?;
    }

    // Pages
    let b = |t: &str, x: &str| json!({ "id": id(), "type": t, "text": x });
    let rules_blocks = json!([
        b("h2", "Règles de la map"), b("p", "Deux équipes de 8. Les voleurs doivent vider le coffre de la banque et s’enfuir ; les gendarmes doivent les arrêter avant la fin du minuteur (8 minutes)."),
        b("h2", "Tags Verse"), json!({ "id": id(), "type": "code", "lang": "verse", "text": "team_police_tag := class(tag){}\nteam_robber_tag := class(tag){}" }),
        b("h2", "Noms des appareils"), b("bullet", "TRG_ pour les déclencheurs"), b("bullet", "BTN_ pour les boutons"), b("bullet", "SPN_ pour les points d’apparition"),
    ]);
    let eco_blocks = json!([
        b("h2", "Monnaies"),
        b("p", "Les voleurs gagnent du Butin en vidant les coffres ; les gendarmes gagnent des Primes à chaque arrestation. Les deux se dépensent dans la boutique entre deux manches."),
        json!({ "id": "regle-or", "type": "callout", "text": "Règle d’or : un joueur doit pouvoir acheter sa première amélioration en moins de 4 minutes." }),
        json!({ "id": id(), "type": "todo", "text": "Prix validés au playtest 4v4 @Inès", "checked": true }),
        json!({ "id": id(), "type": "todo", "text": "Revoir le coût de l’hélicoptère (#21)", "checked": false }),
        json!({ "id": id(), "type": "toggle", "text": "Pourquoi pas de monnaie premium dans la map ?", "body": "Les offres en jeu passent par les V-Bucks d’Epic. Une monnaie premium maison compliquerait la lecture des prix." }),
        json!({ "id": id(), "type": "code", "lang": "verse", "text": "VaultReward<public> : int = 250\nArrestBounty<public> : int = 120" }),
        json!({ "id": id(), "type": "database", "title": "Offres en jeu", "view": "table",
            "columns": [{"key": "name", "label": "Nom", "type": "text"}, {"key": "status", "label": "Statut", "type": "select"}, {"key": "price", "label": "Prix (V-Bucks)", "type": "number"}, {"key": "owner", "label": "Responsable", "type": "person"}, {"key": "date", "label": "Mise en vente", "type": "date"}],
            "rows": [
                {"id": id(), "name": "Pack Braqueur", "status": "En vente", "price": 800, "owner": "Yanis", "date": day(-45)},
                {"id": id(), "name": "Boost butin ×2 (24 h)", "status": "En vente", "price": 200, "owner": "Yanis", "date": day(-45)},
                {"id": id(), "name": "Masque doré", "status": "En vente", "price": 300, "owner": "Hugo", "date": day(-30)},
                {"id": id(), "name": "Pack Gendarme d’élite", "status": "En préparation", "price": 600, "owner": "Hugo", "date": day(9)},
            ] }),
    ]);
    let mut root = None;
    for (title, icon, blocks, fav, parent) in [
        ("VaultRush · document de conception", "V", json!([b("h1", "Pitch"), b("p", "Gendarmes et voleurs tycoon en 8v8 : braquer la banque, améliorer son équipe, s’évader en hélicoptère.")]), 0, false),
        ("Règles et conventions", "R", rules_blocks, 1, true),
        ("Économie", "€", eco_blocks, 1, true),
        ("Journal des décisions", "J", json!([json!({ "id": id(), "type": "table", "rows": [["Date", "Décision", "Pourquoi"], [day(-12), "Manches de 8 minutes", "Les playtests à 12 min perdaient la moitié des joueurs"]] })]), 0, true),
    ] {
        let p: i64 = sqlx::query_scalar("INSERT INTO pages (map_id, parent_id, title, icon, blocks, favorite, created_by, updated_by, version) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1) RETURNING id")
            .bind(mid).bind(if parent { root } else { None }).bind(title).bind(icon).bind(blocks.to_string()).bind(fav).bind(uid["H"]).bind(uid["H"]).fetch_one(&st.db).await?;
        sqlx::query("INSERT INTO page_versions (page_id, version, title, blocks, user_id, sha256, at) VALUES (?, 1, ?, ?, ?, ?, datetime('now', '-60 days'))")
            .bind(p).bind(title).bind(blocks.to_string()).bind(uid["H"]).bind(crate::auth::sha256_hex(&blocks.to_string())).execute(&st.db).await?;
        if root.is_none() {
            root = Some(p);
        }
        if title == "Économie" {
            sqlx::query("INSERT INTO comments (map_id, target, user_id, body) VALUES (?, ?, ?, ?)").bind(mid).bind(format!("page:{p}:regle-or")).bind(uid["H"]).bind("« 4 minutes » : c’est mesuré en playtest ou c’est une cible ?").execute(&st.db).await?;
            sqlx::query("INSERT INTO comments (map_id, target, user_id, body) VALUES (?, ?, ?, ?)").bind(mid).bind(format!("page:{p}:regle-or")).bind(uid["I"]).bind("@Hugo mesuré au 4v4 : médiane 3 min 40.").execute(&st.db).await?;
        }
    }

    // Tableau blanc « Plan de la map »
    let mut objs = serde_json::Map::new();
    let mut put = |o: serde_json::Value| { objs.insert(o["id"].as_str().unwrap().to_string(), o); };
    put(json!({"id": "f-banque", "kind": "frame", "x": 300, "y": 40, "w": 440, "h": 300, "text": "Banque"}));
    put(json!({"id": "f-comm", "kind": "frame", "x": 820, "y": 380, "w": 340, "h": 240, "text": "Commissariat"}));
    put(json!({"id": "f-port", "kind": "frame", "x": 20, "y": 400, "w": 260, "h": 240, "text": "Fuite par le port"}));
    for (oid, x, y, text, shade, votes, author, task) in [
        ("s1", 330, 90, "Tunnel secret sous la banque", 0, vec![("C", 2), ("Y", 1), ("H", 2)], "C", None),
        ("s2", 510, 90, "Alarme à 50 % du butin volé", 1, vec![("I", 2), ("Y", 2)], "H", Some(30)),
        ("s3", 330, 210, "Hélico d’évasion sur le toit", 1, vec![("Y", 2), ("C", 1), ("H", 1), ("I", 1)], "Y", Some(21)),
        ("s4", 510, 210, "Coffre en 3 étapes (perceuse)", 2, vec![("I", 1)], "I", None),
        ("s5", 850, 430, "Drone de surveillance pour les gendarmes", 3, vec![("H", 1)], "H", None),
        ("s6", 1010, 430, "Cellules avec mini-jeu d’évasion", 2, vec![], "C", None),
        ("s7", 60, 450, "Route de fuite par le port en bateau", 0, vec![("I", 1), ("C", 1)], "I", None),
    ] {
        let mut v = serde_json::Map::new();
        for (k, n) in votes { v.insert(uid[k].to_string(), json!(n)); }
        let mut o = json!({"id": oid, "kind": "sticky", "x": x, "y": y, "w": 160, "h": 110, "text": text, "shade": shade, "votes": v, "author": author_name(author), "author_id": uid[author]});
        if let Some(n) = task { o["task_id"] = json!(tid[&n]); o["task_number"] = json!(n); }
        put(o);
    }
    for (oid, shape, x, y, text) in [("u1", "spawn", 60, 120, "Apparition voleurs"), ("u2", "spawn", 1190, 520, "Apparition gendarmes"), ("u3", "camera", 680, 60, "Caméra"), ("u4", "loot", 680, 150, "Butin"), ("u5", "vehicle", 900, 300, "Véhicule")] {
        put(json!({"id": oid, "kind": "uefn", "shape": shape, "x": x, "y": y, "w": 36, "h": 36, "text": text}));
    }
    put(json!({"id": "c1", "kind": "connector", "from": "s3", "to": "f-port", "style": "curve", "arrow": true}));
    put(json!({"id": "c2", "kind": "connector", "from": "u1", "to": "f-banque", "style": "curve", "arrow": true, "dashed": true}));
    put(json!({"id": "__meta", "points": 5, "hidden": false, "timer_end": null}));
    sqlx::query("UPDATE boards SET objects = ? WHERE map_id = ? AND title = 'Plan de la map'").bind(serde_json::Value::Object(objs).to_string()).bind(mid).execute(&st.db).await?;

    // Activité
    for (who, kind, title, detail, ago) in [
        ("Y", "done", "#10 Fait", "Prison et système d’arrestation", "-2 days"), ("I", "playtest", "Playtest à crédits terminé", "6 retours · décrochage surtout à 4 min", "-26 hours"),
        ("Y", "blocked", "#21 Bloqué", "Hélicoptère d’évasion attend #18", "-5 hours"), ("H", "page", "Page modifiée", "Économie", "-3 hours"),
        ("H", "board", "2 post-it convertis en tâches", "Depuis le tableau blanc", "-40 minutes"), ("C", "checkin", "Check-in Lore de Chloé", "VaultRush_City.umap · #15", "-12 minutes"),
    ] {
        sqlx::query("INSERT INTO activity (map_id, user_id, kind, title, detail, at) VALUES (?, ?, ?, ?, ?, datetime('now', ?))").bind(mid).bind(uid[who]).bind(kind).bind(title).bind(detail).bind(ago).execute(&st.db).await?;
    }

    // Gains : accès anticipé, 2 mois
    for (m, e, s) in [(month(-2), 41200, 26800), (month(-1), 98600, 64200)] {
        sqlx::query("INSERT INTO revenues (map_id, month, engagement_cents, sales_cents, source) VALUES (?, ?, ?, ?, 'csv')").bind(mid).bind(m).bind(e).bind(s).execute(&st.db).await?;
    }
    for (who, kind, v) in [(Some("H"), "fixed", 25000), (Some("I"), "per_task", 2000), (Some("Y"), "percent", 5000), (Some("C"), "percent", 4000), (None, "percent", 1000)] {
        sqlx::query("INSERT INTO share_rules (map_id, user_id, kind, value) VALUES (?, ?, ?, ?)").bind(mid).bind(who.map(|w| uid[w])).bind(kind).bind(v).execute(&st.db).await?;
    }
    for (name, price, s1, r1, s2, r2) in [("Pack Braqueur", 800, 20, 12000, 52, 31200), ("Boost butin ×2 (24 h)", 200, 46, 9200, 105, 21000), ("Masque doré", 300, 19, 5600, 40, 12000)] {
        let o: i64 = sqlx::query_scalar("INSERT INTO offers (map_id, name, price_vbucks) VALUES (?, ?, ?) RETURNING id").bind(mid).bind(name).bind(price).fetch_one(&st.db).await?;
        sqlx::query("INSERT INTO offer_sales (offer_id, month, sales, revenue_cents) VALUES (?, ?, ?, ?), (?, ?, ?, ?)").bind(o).bind(month(-2)).bind(s1).bind(r1).bind(o).bind(month(-1)).bind(s2).bind(r2).execute(&st.db).await?;
    }

    // Lancer : checklist et playtests
    for (k, s, note) in [("title", "ok", ""), ("thumbnail", "warn", "Le mot « FREE » apparaît sur la vignette."), ("description", "ok", ""), ("tags", "warn", "« xp » ne correspond pas au gameplay."), ("rating", "block", "Questionnaire non commencé."), ("notes", "todo", ""), ("memory", "ok", "")] {
        sqlx::query("UPDATE publish_checks SET status = ?, note = ? WHERE map_id = ? AND key = ?").bind(s).bind(note).bind(mid).bind(k).execute(&st.db).await?;
    }
    let lid: i64 = sqlx::query_scalar("INSERT INTO playtest_listings (map_id, owner_id, minutes, reward, slots, island_code) VALUES (?, ?, 15, 20, 2, '1234-5678-9012') RETURNING id").bind(mid).bind(uid["Y"]).fetch_one(&st.db).await?;
    for (k, minute, reason, und, cb, score) in [
        ("K", 4, "Premier coffre trop loin", "En partie", "Un premier coffre plus près", 4), ("P", 3, "Je ne savais pas quoi faire ensuite", "Non", "Une flèche vers la banque", 3),
        ("S", 15, "Fin de session", "Oui", "Le boss de la mine", 5),
    ] {
        sqlx::query("INSERT INTO playtest_feedback (listing_id, tester_id, understood, left_minute, reason, comeback, score) VALUES (?, ?, ?, ?, ?, ?, ?)").bind(lid).bind(uid[k]).bind(und).bind(minute).bind(reason).bind(cb).bind(score).execute(&st.db).await?;
    }
    // Un autre studio propose sa map à tester (pour gagner des crédits).
    let other: i64 = sqlx::query_scalar("INSERT INTO studios (name) VALUES ('Kiro Lab') RETURNING id").fetch_one(&st.db).await?;
    sqlx::query("INSERT INTO memberships (studio_id, user_id, role) VALUES (?, ?, 'owner')").bind(other).bind(uid["K"]).execute(&st.db).await?;
    let omid = insert_map(st, other, "Neon Box Arena", "Box fight", None).await?;
    sqlx::query("INSERT INTO playtest_listings (map_id, owner_id, minutes, reward, slots, island_code) VALUES (?, ?, 10, 25, 5, '4821-0193-5573')").bind(omid).bind(uid["K"]).execute(&st.db).await?;

    sqlx::query("INSERT INTO integrations (studio_id, kind, enabled, config) VALUES (?, 'discord', 1, ?), (?, 'lore', 1, ?)")
        .bind(sid).bind(json!({"channel": "#vaultrush-dev", "report_time": "21:00"}).to_string())
        .bind(sid).bind(json!({"token": crate::auth::random_token()}).to_string())
        .execute(&st.db).await?;

    println!("Données d'exemple créées. Connexion : yanis@demo.pop / {DEMO_PASSWORD}");
    Ok(())
}

fn author_name(k: &str) -> &'static str {
    match k { "Y" => "Yanis Belkacem", "C" => "Chloé Martin", "H" => "Hugo Lefèvre", _ => "Inès Diallo" }
}
