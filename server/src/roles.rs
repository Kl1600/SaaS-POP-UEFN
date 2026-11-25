//! Rôles des agents IA (#111) : un rôle = des consignes envoyées à l'IA à la connexion MCP + la liste de ses outils.
//! Un agent sans rôle garde tous les outils (comportement d'avant). Le rôle se lit à chaque appel : le changer agit
//! au prochain appel. Un outil hors rôle est refusé avec un message clair.

/// Outils de tous les rôles : choisir sa map, lire, et suivre les tâches.
const COMMUNS: &[&str] = &[
    "list_maps",
    "use_map",
    "ask_user",
    "get_answer",
    "get_feasibility",
    "uefn_status",
    "check_level",
    "list_actors",
    "get_actor",
    "list_verse_files",
    "read_verse_file",
    "session_output",
    "list_tasks",
    "get_task",
    "next_task",
    "search_library",
    "get_library_module",
    "list_skills",
    "get_skill",
    "comment_task",
    "list_agents",
    "list_boards",
    "get_board",
    "list_pages",
    "read_page",
    "list_milestones",
    "list_scenarios",
    "list_device_configs",
    "search_devices",
    "device_graph",
    "read_messages",
    "send_message",
    "forge_help",
];

/// Ce qu'un agent fait de ses tâches : réserver, passer en cours ou fini, signaler un problème par une tâche.
const TACHES: &[&str] = &[
    "claim_task",
    "release_task",
    "save_progress",
    "set_task_status",
    "create_task",
    "propose_scenario",
];

/// Conception de la map (#147) : tableau blanc, documentation, jalons.
const CONCEPTION: &[&str] = &[
    "create_board",
    "upsert_board_objects",
    "delete_board_objects",
    "write_page",
    "add_milestone",
];

const SESSION: &[&str] = &[
    "compile_verse",
    "start_session",
    "stop_session",
    "push_to_session",
];

pub struct Role {
    pub id: &'static str,
    pub nom: &'static str,
    pub consignes: &'static str,
    /// Outils en plus des communs.
    pub outils: &'static [&'static [&'static str]],
}

pub const ROLES: &[Role] = &[
    Role {
        id: "orchestrateur",
        nom: "Orchestrateur",
        consignes: "Vous êtes l'Orchestrateur de l'équipe. Vous ne codez pas et ne touchez pas au niveau : vous transformez la demande de l'humain en tâches POP d'environ une heure, liées par leurs dépendances (plan_tasks), vous les attribuez aux agents selon leur rôle (list_agents, assign_task), vous suivez l'avancement (list_tasks) et vous relisez ce que les autres agents ont fait (commentaires des tâches). Quand une tâche bloque, commentez-la et prévenez l'humain.",
        outils: &[TACHES, &["set_task_lot"], crate::orchestre::OUTILS, CONCEPTION],
    },
    Role {
        id: "codeur_verse",
        nom: "Codeur Verse",
        consignes: "Vous êtes le Codeur Verse. Vous écrivez et corrigez le Verse de la map (write_verse_file), puis vous compilez (compile_verse) jusqu'à zéro erreur avant de dire qu'une tâche est finie. Lisez la compétence « verse » et la bibliothèque du studio avant d'écrire du neuf. Commentez le code en français, avec des noms lisibles. Ne touchez pas au placement des acteurs : c'est le rôle du Level designer.",
        outils: &[TACHES, SESSION, &["write_verse_file", "create_widget", "undo_action"]],
    },
    Role {
        id: "level_designer",
        nom: "Level designer",
        consignes: "Vous êtes le Level designer. Vous posez, réglez, rangez et dupliquez les acteurs et appareils du niveau, et vous importez les fichiers nécessaires. Lisez la compétence « appareils » avant de régler une option. Vérifiez le niveau (check_level) avant de dire qu'une tâche est finie. Ne modifiez pas le Verse : c'est le rôle du Codeur Verse.",
        outils: &[
            TACHES,
            SESSION,
            &[
                "update_actors",
                "duplicate_actors",
                "delete_actors",
                "place_device",
                "scatter_props",
                "capture_island",
                "place_mesh",
                "save_device_config",
                "apply_device_config",
                "place_configured_device",
                "blockout",
                "set_island_settings",
                "get_island_settings",
                "import_asset",
                "save_level",
                "undo_action",
            ],
        ],
    },
    Role {
        id: "ui",
        nom: "UI",
        consignes: "Vous êtes chargé de l'interface de jeu (HUD, menus, boutons). Créez les écrans avec create_widget, ajustez leur Verse si besoin, importez les images nécessaires, puis compilez et testez l'affichage en session. Gardez des textes courts et lisibles.",
        outils: &[
            TACHES,
            SESSION,
            &["create_widget", "write_verse_file", "import_asset", "undo_action"],
        ],
    },
    Role {
        id: "testeur",
        nom: "Testeur",
        consignes: "Vous êtes le Testeur. Vous prenez les tâches prêtes à tester : vous compilez, lancez une session, envoyez les changements, lisez la sortie du jeu (session_output) et vérifiez le niveau (check_level). Ne dites jamais qu'un test a réussi sans avoir lu la sortie de la dernière version compilée. Un problème trouvé devient un commentaire sur la tâche, ou une nouvelle tâche s'il est à part. Vous ne modifiez ni le Verse ni le niveau.",
        outils: &[TACHES, SESSION],
    },
    Role {
        id: "relecteur",
        nom: "Relecteur",
        consignes: "Vous êtes le Relecteur, en lecture seule. Vous lisez le Verse, le niveau et les tâches, et vous donnez votre avis par des commentaires sur les tâches (comment_task) : erreurs, risques, ce qui manque. Vous ne modifiez rien.",
        outils: &[],
    },
    Role {
        id: "analyste",
        nom: "Analyste",
        consignes: "Vous êtes l'Analyste. Vous lisez la map, ses tâches et le niveau pour repérer ce qui manque ou ce qui coince, et vous le dites par des commentaires sur les tâches. Vous ne modifiez rien.",
        outils: &[],
    },
];

pub fn get(id: &str) -> Option<&'static Role> {
    ROLES.iter().find(|r| r.id == id)
}

/// L'outil est-il permis à ce rôle ? Un agent sans rôle (ou un rôle inconnu) a tous les outils.
pub fn allows(role: Option<&str>, tool: &str) -> bool {
    match role.and_then(get) {
        // Sans rôle : tout, sauf l'orchestration, que mcp.rs n'ouvre qu'à l'Orchestrateur par défaut de la map.
        None => !crate::orchestre::OUTILS.contains(&tool),
        Some(r) => COMMUNS.contains(&tool) || r.outils.iter().any(|g| g.contains(&tool)),
    }
}

/// Message de refus, en français, pour l'IA comme pour l'humain qui lit le journal.
pub fn refus(role: &str, tool: &str) -> String {
    let nom = get(role).map(|r| r.nom).unwrap_or(role);
    format!("Le rôle « {nom} » de cet agent ne permet pas {tool}. Laissez cette partie à un agent du bon rôle, ou demandez à un humain de changer le rôle de cet agent (Paramètres → IA connectées).")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn droits() {
        // Sans rôle : tout, sauf l'orchestration.
        assert!(allows(None, "delete_actors") && !allows(None, "assign_task"));
        assert!(
            allows(Some("orchestrateur"), "plan_tasks")
                && !allows(Some("codeur_verse"), "assign_task")
        );
        // Le Relecteur ne modifie rien.
        for t in [
            "write_verse_file",
            "update_actors",
            "delete_actors",
            "set_task_status",
            "create_task",
            "compile_verse",
            "undo_action",
        ] {
            assert!(!allows(Some("relecteur"), t), "{t}");
        }
        assert!(
            allows(Some("relecteur"), "read_verse_file")
                && allows(Some("relecteur"), "comment_task")
        );
        assert!(
            allows(Some("codeur_verse"), "write_verse_file")
                && !allows(Some("codeur_verse"), "place_device")
        );
        assert!(
            allows(Some("level_designer"), "place_device")
                && !allows(Some("level_designer"), "write_verse_file")
        );
        assert!(
            !allows(Some("orchestrateur"), "write_verse_file")
                && allows(Some("orchestrateur"), "create_task")
        );
        // Chaque outil nommé dans un rôle existe dans le serveur MCP.
        let tous: Vec<String> = crate::mcp::tool_names();
        for r in ROLES {
            for t in r.outils.iter().flat_map(|g| g.iter()).chain(COMMUNS) {
                assert!(tous.iter().any(|x| x == t), "{} : {t} inconnu", r.id);
            }
        }
    }
}
