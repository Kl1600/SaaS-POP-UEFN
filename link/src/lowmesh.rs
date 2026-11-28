//! Outil Low Mesh (#325) : alléger un modèle 3D (FBX, OBJ, glTF) avant de l'importer dans UEFN, par le modificateur
//! Decimate de Blender (#351). Aperçu et nombre de triangles et de sommets avant et après ; export en FBX.
use anyhow::{bail, Result};

/// Le validateur de mesh d'Epic avertit au-delà de ce nombre de sommets.
pub const SOMMETS_MAX_CONSEILLES: u64 = 30_000;
/// Taille maximale du modèle d'entrée.
pub const ENTREE_MAX: usize = 100 << 20;
const EXT: &[&str] = &["fbx", "obj", "glb", "gltf"];

/// Vérifie le fichier et le taux, et rend le script et ses fichiers d'entrée.
pub fn script(nom: &str, donnees: Vec<u8>, taux: f64) -> Result<(String, Vec<(String, Vec<u8>)>)> {
    let ext = nom.rsplit('.').next().unwrap_or("").to_lowercase();
    if !nom.contains('.') || !EXT.contains(&ext.as_str()) {
        bail!("Format non pris en charge : FBX, OBJ, GLB ou glTF.");
    }
    if donnees.is_empty() || donnees.len() > ENTREE_MAX {
        bail!("Modèle vide ou trop gros (100 Mo au plus).");
    }
    if !(0.01..=1.0).contains(&taux) {
        bail!("Taux de réduction invalide.");
    }
    let fichier = format!("modele.{ext}");
    let reglages = serde_json::json!({ "fichier": fichier, "taux": taux });
    Ok((
        SCRIPT.to_string(),
        vec![
            ("reglages.json".into(), reglages.to_string().into_bytes()),
            (fichier, donnees),
        ],
    ))
}

const SCRIPT: &str = r#"
import bpy, os, json, math
from mathutils import Vector
P = json.load(open(os.path.join(TRAVAIL, "reglages.json"), encoding="utf-8"))
f = os.path.join(TRAVAIL, P["fichier"])
ext = f.rsplit(".", 1)[1].lower()
if ext == "fbx":
    bpy.ops.import_scene.fbx(filepath=f)
elif ext == "obj":
    bpy.ops.wm.obj_import(filepath=f)
else:
    bpy.ops.import_scene.gltf(filepath=f)
sc = bpy.context.scene
for o in list(sc.objects):
    if o.type not in ("MESH", "EMPTY"):
        bpy.data.objects.remove(o)
objets = [o for o in sc.objects if o.type == "MESH"]
if not objets:
    raise SystemExit("Le fichier ne contient aucun mesh.")
for o in objets:
    for m in list(o.modifiers):
        if m.type == "ARMATURE":
            o.modifiers.remove(m)

def compter():
    dg = bpy.context.evaluated_depsgraph_get()
    t = s = 0
    for o in objets:
        e = o.evaluated_get(dg)
        m = e.to_mesh()
        m.calc_loop_triangles()
        t += len(m.loop_triangles)
        s += len(m.vertices)
        e.to_mesh_clear()
    return t, s

def apercu(chemin):
    dg = bpy.context.evaluated_depsgraph_get()
    pts = []
    for o in objets:
        e = o.evaluated_get(dg)
        pts += [e.matrix_world @ Vector(c) for c in e.bound_box]
    lo = Vector((min(p.x for p in pts), min(p.y for p in pts), min(p.z for p in pts)))
    hi = Vector((max(p.x for p in pts), max(p.y for p in pts), max(p.z for p in pts)))
    c = (lo + hi) / 2
    r = max((hi - lo).length / 2, 0.01)
    cam = bpy.data.objects.new("pop_avant", bpy.data.cameras.new("pop_avant"))
    sc.collection.objects.link(cam)
    d = Vector((1.0, -1.2, 0.8)).normalized()
    cam.location = c + d * r / math.sin(math.radians(20)) * 1.15
    cam.rotation_euler = (-d).to_track_quat("-Z", "Y").to_euler()
    cam.data.angle = math.radians(40)
    cam.data.clip_end = r * 100
    sol = bpy.data.objects.new("pop_avant_sol", bpy.data.lights.new("pop_avant_sol", "SUN"))
    sol.data.energy = 3
    sol.rotation_euler = (math.radians(50), 0, math.radians(30))
    sc.collection.objects.link(sol)
    sc.camera = cam
    sc.render.engine = "CYCLES"
    sc.cycles.device = "CPU"
    sc.cycles.samples = 8
    sc.render.resolution_x = sc.render.resolution_y = 384
    sc.render.film_transparent = True
    sc.render.image_settings.file_format = "PNG"
    sc.render.filepath = chemin
    bpy.ops.render.render(write_still=True)
    sc.camera = None
    bpy.data.objects.remove(cam)
    bpy.data.objects.remove(sol)

avant = compter()
apercu(os.path.join(TRAVAIL, "sortie.avant.png"))
if P["taux"] < 1:
    for o in objets:
        bpy.context.view_layer.objects.active = o
        m = o.modifiers.new("POP_Decimate", "DECIMATE")
        m.ratio = P["taux"]
        bpy.ops.object.modifier_apply(modifier=m.name)
apres = compter()
bpy.ops.export_scene.fbx(filepath=os.path.join(TRAVAIL, "sortie.fbx"), object_types={"MESH", "EMPTY"}, mesh_smooth_type="FACE")
json.dump({"avant": {"triangles": avant[0], "sommets": avant[1]}, "apres": {"triangles": apres[0], "sommets": apres[1]}}, open(os.path.join(TRAVAIL, "sortie.json"), "w"))
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fichier_et_taux_verifies() {
        assert!(script("a.blend", vec![1], 0.5).is_err());
        assert!(script("a.fbx", vec![], 0.5).is_err());
        assert!(script("a.fbx", vec![1], 0.0).is_err());
        let (s, e) = script("Arbre Géant.GLB", vec![1, 2], 0.25).unwrap();
        assert_eq!(e[1].0, "modele.glb");
        assert!(s.contains("DECIMATE"));
        let r: serde_json::Value = serde_json::from_slice(&e[0].1).unwrap();
        assert_eq!(r["taux"], 0.25);
    }
}
