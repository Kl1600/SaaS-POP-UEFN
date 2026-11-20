//! UEFN Link : relais local entre UEFN (sur le PC du créateur) et POP. Ligne de commande de `pop-link.exe` ;
//! tout le relais est dans la bibliothèque (`lib.rs`), partagée avec POP Forge.
//!
//!   pop-link                      (double-clic : appairage guidé la première fois, puis connexion)
//!   pop-link pair <adresse POP> <code> <dossier du projet UEFN> [--name "PC de Yanis"]
//!   pop-link run [--port 47800]
//!   pop-link diagnostic           (vérifie UEFN MCP et liste ses outils)
use pop_link::{auto_pair, diagnostic, embedded, guided_pair, load, pair, pause, run};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let flag = |f: &str| {
        args.iter()
            .position(|a| a == f)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let port = flag("--port").and_then(|p| p.parse().ok()).unwrap_or(47800);
    let r = match args.get(1).map(String::as_str) {
        Some("pair") if args.len() >= 5 => {
            pair(
                &args[2],
                &args[3],
                &args[4],
                &flag("--name").unwrap_or_else(|| "UEFN Link".into()),
            )
            .await
        }
        Some("diagnostic") => {
            let r = diagnostic().await;
            if let Err(e) = &r {
                println!("{e:#}")
            }
            pause();
            Ok(())
        }
        Some("run") => run(port).await,
        None => {
            if load().is_err() {
                let r = match embedded() {
                    Some(emb) => auto_pair(&emb).await,
                    None => guided_pair().await,
                };
                if let Err(e) = r {
                    println!("\nLiaison impossible : {e:#}");
                    pause();
                    return;
                }
            }
            run(port).await
        }
        _ => {
            println!("UEFN Link\n  pop-link                 appairage guidé puis connexion\n  pop-link pair <adresse POP> <code> <dossier du projet UEFN> [--name \"PC de Yanis\"]\n  pop-link run [--port 47800]\n  pop-link diagnostic      vérifie UEFN MCP");
            Ok(())
        }
    };
    if let Err(e) = r {
        println!("Erreur : {e:#}");
        pause();
    }
}
