//! Outil Texte 3D et 2D (#324) : un titre de map, un panneau ou un logo, fait par Blender (#351).
//! - 3D : lettres extrudées et biseautées, converties en mesh, exportées en FBX (le format d'import vérifié, #90).
//! - 2D : rendu PNG transparent, de face, à la couleur choisie.
//! Polices libres seulement (licence OFL) : Poppins et Anton, les mêmes fichiers que l'aperçu 3D de Forge (#514).
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

const POPPINS: &[u8] = include_bytes!("../../server/assets/polices/Poppins-Bold.ttf");
const ANTON: &[u8] = include_bytes!("../../server/assets/polices/Anton-Regular.ttf");

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Params {
    pub texte: String,
    /// « 3d » ou « 2d ».
    pub mode: String,
    /// « poppins » ou « anton ».
    pub police: String,
    /// Hauteur des lettres, en mètres.
    pub taille: f64,
    /// Épaisseur (3D), en mètres.
    pub epaisseur: f64,
    /// Biseau, en mètres.
    pub biseau: f64,
    /// Couleur « #rrggbb ».
    pub couleur: String,
}

/// Police et licence, à noter avec l'asset.
pub fn licence(police: &str) -> &'static str {
    match police {
        "anton" => "Anton (The Anton Project Authors), SIL Open Font License 1.1",
        _ => "Poppins (Indian Type Foundry), SIL Open Font License 1.1",
    }
}

fn couleur(c: &str) -> Option<[f64; 3]> {
    let h = c.strip_prefix('#')?;
    if h.len() != 6 || !h.chars().all(|x| x.is_ascii_hexdigit()) {
        return None;
    }
    let v = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap() as f64 / 255.0;
    // sRGB → linéaire (les couleurs de Blender sont linéaires).
    let lin = |x: f64| {
        if x <= 0.04045 {
            x / 12.92
        } else {
            ((x + 0.055) / 1.055).powf(2.4)
        }
    };
    Some([lin(v(0)), lin(v(2)), lin(v(4))])
}

/// Vérifie les réglages et rend le script Blender et ses fichiers d'entrée (réglages en JSON, police).
pub fn script(p: &Params) -> Result<(String, Vec<(String, Vec<u8>)>, &'static str)> {
    let texte = p.texte.trim();
    if texte.is_empty() || texte.chars().count() > 60 {
        bail!("Le texte doit faire de 1 à 60 caractères.");
    }
    let ext = match p.mode.as_str() {
        "3d" => "fbx",
        "2d" => "png",
        _ => bail!("Mode inconnu : 3d ou 2d."),
    };
    if !["poppins", "anton"].contains(&p.police.as_str()) {
        bail!("Police inconnue.");
    }
    let rgb =
        couleur(&p.couleur).ok_or_else(|| anyhow::anyhow!("Couleur invalide (ex. #ffcc00)."))?;
    let reglages = serde_json::json!({
        "texte": texte,
        "mode": p.mode,
        "police": p.police,
        "taille": p.taille.clamp(0.2, 20.0),
        "epaisseur": p.epaisseur.clamp(0.0, 2.0),
        "biseau": p.biseau.clamp(0.0, 0.2),
        "couleur": rgb,
    });
    let mut entrees = vec![(
        "reglages.json".to_string(),
        reglages.to_string().into_bytes(),
    )];
    entrees.push((
        "police.ttf".into(),
        if p.police == "anton" { ANTON } else { POPPINS }.to_vec(),
    ));
    Ok((SCRIPT.to_string(), entrees, ext))
}

/// Le texte et les réglages ne sont jamais écrits dans le code : le script les lit dans `reglages.json`.
const SCRIPT: &str = r#"
import bpy, json, os, math
P = json.load(open(os.path.join(TRAVAIL, "reglages.json"), encoding="utf-8"))
d3 = P["mode"] == "3d"
cu = bpy.data.curves.new("Texte", "FONT")
cu.body = P["texte"]
cu.font = bpy.data.fonts.load(os.path.join(TRAVAIL, "police.ttf"))
cu.align_x = "CENTER"
cu.align_y = "CENTER"
cu.size = P["taille"]
cu.resolution_u = 6
# Même forme que l'aperçu de Forge (#514) : épaisseur totale = épaisseur, biseau compris.
cu.bevel_depth = min(P["biseau"], P["epaisseur"] / 2) if d3 else 0
cu.extrude = max(P["epaisseur"] / 2 - cu.bevel_depth, 0) if d3 else 0
cu.bevel_resolution = 3
o = bpy.data.objects.new("Texte", cu)
bpy.context.scene.collection.objects.link(o)
m = bpy.data.materials.new("Texte")
m.use_nodes = True
b = m.node_tree.nodes.get("Principled BSDF")
r, g, v = P["couleur"]
b.inputs["Base Color"].default_value = (r, g, v, 1)
if not d3:
    b.inputs["Emission Color"].default_value = (r, g, v, 1)
    b.inputs["Emission Strength"].default_value = 1.0
cu.materials.append(m)
bpy.context.view_layer.objects.active = o
o.select_set(True)
bpy.ops.object.convert(target="MESH")
o = bpy.context.view_layer.objects.active
if d3:
    # Debout, face à -Y (vue de face dans UEFN après import).
    o.rotation_euler = (math.radians(90), 0, 0)
    bpy.ops.object.transform_apply(rotation=True)
    bpy.ops.export_scene.fbx(filepath=os.path.join(TRAVAIL, "sortie.fbx"), use_selection=True, mesh_smooth_type="FACE")
else:
    sc = bpy.context.scene
    lo = [min(c[i] for c in o.bound_box) for i in range(3)]
    hi = [max(c[i] for c in o.bound_box) for i in range(3)]
    w, h = hi[0] - lo[0], hi[1] - lo[1]
    cam = bpy.data.objects.new("vue", bpy.data.cameras.new("vue"))
    cam.data.type = "ORTHO"
    cam.data.ortho_scale = max(w, h) * 1.08
    cam.location = ((lo[0] + hi[0]) / 2, (lo[1] + hi[1]) / 2, 10)
    sc.collection.objects.link(cam)
    sc.camera = cam
    sc.render.engine = "CYCLES"
    sc.cycles.device = "CPU"
    sc.cycles.samples = 8
    px = 2048
    sc.render.resolution_x = px if w >= h else max(int(px * w / h), 16)
    sc.render.resolution_y = px if h > w else max(int(px * h / w), 16)
    sc.render.resolution_percentage = 100
    sc.render.film_transparent = True
    sc.view_settings.view_transform = "Standard"
    sc.render.image_settings.file_format = "PNG"
    sc.render.image_settings.color_mode = "RGBA"
    sc.render.filepath = os.path.join(TRAVAIL, "sortie.png")
    bpy.ops.render.render(write_still=True)
    sc.camera = None
    bpy.data.objects.remove(cam)
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn p() -> Params {
        Params {
            texte: "GrassCutter".into(),
            mode: "3d".into(),
            police: "poppins".into(),
            taille: 2.0,
            epaisseur: 0.4,
            biseau: 0.03,
            couleur: "#FFCC00".into(),
        }
    }

    #[test]
    fn reglages_hors_du_code() {
        let mut x = p();
        x.texte = "\"\"\"; import os; os.system('x') #".into();
        let (s, e, ext) = script(&x).unwrap();
        assert_eq!(ext, "fbx");
        assert!(
            !s.contains("os.system"),
            "le texte ne va jamais dans le script"
        );
        let r: serde_json::Value = serde_json::from_slice(&e[0].1).unwrap();
        assert_eq!(r["texte"], x.texte.trim());
        assert_eq!(e[1].0, "police.ttf");
        assert!((r["couleur"][0].as_f64().unwrap() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn reglages_verifies() {
        let mut x = p();
        x.texte = "".into();
        assert!(script(&x).is_err());
        let mut x = p();
        x.couleur = "rouge".into();
        assert!(script(&x).is_err());
        let mut x = p();
        x.mode = "2d".into();
        x.police = "anton".into();
        let (_, e, ext) = script(&x).unwrap();
        assert_eq!((ext, e.len()), ("png", 2));
        x.police = "inter".into();
        assert!(script(&x).is_err());
        assert!(licence("poppins").contains("Open Font License"));
    }
}
