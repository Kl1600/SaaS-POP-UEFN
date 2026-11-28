// Plan 2D de la map, étage par étage, à l'échelle (#162). Le serveur garde tout en centimètres, comme le panneau
// Détails d'UEFN (X vers l'avant, Y vers la droite) ; l'écran montre X vers le haut et Y vers la droite, avec une
// grille en mètres, une règle, les étages (les autres en transparence) et le calque « niveau réel » du relais.
import { useEffect, useMemo, useRef, useState } from 'react'
import { Circle, Hexagon, Layers, MousePointer2, Plus, Route, Ruler, Square, Trash2, Minus as Wall, MapPin } from '../icons'
import { api } from '../api'
import { useApp, useLive, useLoad } from '../state'
import { tr } from '../i18n/t'

type Floor = { id: number; name: string; z_min: number; z_max: number; position: number }
type Obj = { id: number; floor_id: number; kind: string; name: string; device: string; x: number; y: number; w: number; h: number; rotation: number; points: [number, number][] }
type Outil = 'select' | 'zone' | 'cercle' | 'polygone' | 'chemin' | 'mur' | 'appareil' | 'regle'

const APPAREILS: [string, string][] = [
  ['apparition', tr("Apparition")], ['vente', tr("Vente")], ['boutique', tr("Boutique")], ['barriere', tr("Barrière")], ['generateur', tr("Générateur")],
  ['bouton', tr("Bouton")], ['declencheur', tr("Déclencheur")], ['objectif', tr("Objectif")], ['camera', tr("Caméra")], ['porte', tr("Porte")],
]
const OUTILS: [Outil, string, any][] = [
  ['select', tr("Sélection"), MousePointer2], ['zone', tr("Zone"), Square], ['cercle', tr("Cercle"), Circle], ['polygone', tr("Polygone"), Hexagon],
  ['chemin', tr("Chemin"), Route], ['mur', tr("Mur"), Wall], ['appareil', tr("Appareil"), MapPin], ['regle', tr("Règle"), Ruler],
]
const m = (cm: number) => Math.round(cm) / 100

/** Distance en cm → mètres à deux décimales, au format de la langue (« 20,00 » en français). */
const metres = (cm: number) => new Intl.NumberFormat(document.documentElement.lang || 'fr', { minimumFractionDigits: 2, maximumFractionDigits: 2 }).format(cm / 100)

/** Plan 2D, affiché dans POP Forge (#430) : section « Plan 2D », sur la map courante. */
export default function Plan2D() {
  const { map, toast } = useApp()
  const mid = map?.id
  const { data, reload } = useLoad<{ floors: Floor[]; objects: Obj[] }>(mid ? `/api/maps/${mid}/plan2d` : null, [mid])
  const [niveauOn, setNiveauOn] = useState(false)
  const { data: niveau, reload: reloadNiveau } = useLoad<{ actors: any[] }>(mid && niveauOn ? `/api/maps/${mid}/plan2d/level` : null, [mid, niveauOn])
  useLive(['plan2d'], reload)
  useLive(['uefn'], () => { if (niveauOn) reloadNiveau() })
  const [etage, setEtage] = useState<number | null>(null)
  const [outil, setOutil] = useState<Outil>('select')
  const [appareil, setAppareil] = useState('apparition')
  const [sel, setSel] = useState<number | null>(null)
  const [dossier, setDossier] = useState('')
  // Vue : px par mètre et centre (en cm, coordonnées UEFN).
  const [zoom, setZoom] = useState(8)
  const [centre, setCentre] = useState<[number, number]>([0, 0])
  const [curseur, setCurseur] = useState<[number, number] | null>(null)
  const [trace, setTrace] = useState<[number, number][]>([])
  const [regle, setRegle] = useState<[number, number][]>([])
  const [glisse, setGlisse] = useState<{ mode: 'pan' | 'move' | 'draw'; from: [number, number]; orig?: [number, number] } | null>(null)
  const svg = useRef<SVGSVGElement>(null)
  const [taille, setTaille] = useState<[number, number]>([900, 560])
  useEffect(() => {
    const el = svg.current; if (!el) return
    const ro = new ResizeObserver(() => setTaille([el.clientWidth, el.clientHeight])); ro.observe(el); return () => ro.disconnect()
  }, [])
  useEffect(() => { if (data && (etage === null || !data.floors.some(f => f.id === etage))) setEtage(data.floors[0]?.id ?? null) }, [data])

  // cm (UEFN) → écran : X vers le haut, Y vers la droite.
  const ecran = (x: number, y: number): [number, number] => [taille[0] / 2 + (y - centre[1]) / 100 * zoom, taille[1] / 2 - (x - centre[0]) / 100 * zoom]
  const monde = (sx: number, sy: number): [number, number] => [centre[0] - (sy - taille[1] / 2) / zoom * 100, centre[1] + (sx - taille[0] / 2) / zoom * 100]
  const souris = (e: React.MouseEvent): [number, number] => { const r = svg.current!.getBoundingClientRect(); return monde(e.clientX - r.left, e.clientY - r.top) }
  const arrondi = (p: [number, number]): [number, number] => [Math.round(p[0] / 50) * 50, Math.round(p[1] / 50) * 50] // 50 cm

  const objets = data?.objects ?? []
  const choisi = objets.find(o => o.id === sel) ?? null
  const ecrire = async (fn: () => Promise<any>) => { try { await fn(); reload() } catch (e: any) { toast(e.message, true) } }
  const creer = (o: Partial<Obj>) => ecrire(async () => { const r = await api.post<{ id: number }>(`/api/maps/${mid}/plan2d`, { floor_id: etage, ...o }); setSel(r.id); setOutil('select') })
  const maj = (id: number, o: Partial<Obj>) => ecrire(() => api.patch(`/api/maps/${mid}/plan2d/${id}`, o))

  const down = (e: React.MouseEvent) => {
    const p = souris(e)
    if (outil === 'select') {
      const cible = (e.target as Element).closest('[data-obj]')
      if (cible) { const id = Number(cible.getAttribute('data-obj')); const o = objets.find(x => x.id === id)!; setSel(id); if (!o.points.length) setGlisse({ mode: 'move', from: p, orig: [o.x, o.y] }); return }
      setSel(null); setGlisse({ mode: 'pan', from: p }); return
    }
    if (outil === 'zone' || outil === 'cercle') { setGlisse({ mode: 'draw', from: arrondi(p) }); return }
    if (outil === 'appareil') { const q = arrondi(p); creer({ kind: 'appareil', device: appareil, name: APPAREILS.find(a => a[0] === appareil)?.[1] ?? '', x: q[0], y: q[1], w: 100, h: 100 }); return }
    if (outil === 'regle') { setRegle(r => r.length >= 2 ? [arrondi(p)] : [...r, arrondi(p)]); return }
    setTrace(t => [...t, arrondi(p)])
  }
  const move = (e: React.MouseEvent) => {
    const p = souris(e); setCurseur(p)
    if (!glisse) return
    if (glisse.mode === 'pan') setCentre(c => [c[0] - (p[0] - glisse.from[0]), c[1] - (p[1] - glisse.from[1])])
  }
  const up = (e: React.MouseEvent) => {
    const p = souris(e)
    if (glisse?.mode === 'move' && choisi && glisse.orig) {
      const q = arrondi([glisse.orig[0] + p[0] - glisse.from[0], glisse.orig[1] + p[1] - glisse.from[1]])
      if (q[0] !== choisi.x || q[1] !== choisi.y) maj(choisi.id, { x: q[0], y: q[1] })
    }
    if (glisse?.mode === 'draw') {
      const a = glisse.from, b = arrondi(p)
      if (outil === 'zone' && Math.abs(b[0] - a[0]) >= 100 && Math.abs(b[1] - a[1]) >= 100)
        creer({ kind: 'zone', name: tr("Zone"), x: (a[0] + b[0]) / 2, y: (a[1] + b[1]) / 2, w: Math.abs(b[0] - a[0]), h: Math.abs(b[1] - a[1]) })
      if (outil === 'cercle') { const d = 2 * Math.hypot(b[0] - a[0], b[1] - a[1]); if (d >= 100) creer({ kind: 'cercle', name: tr("Zone"), x: a[0], y: a[1], w: d, h: d }) }
    }
    setGlisse(null)
  }
  const finirTrace = () => {
    if (trace.length >= 2 && (outil === 'polygone' ? trace.length >= 3 : true)) creer({ kind: outil, name: outil === 'mur' ? tr("Mur") : outil === 'chemin' ? tr("Chemin") : tr("Zone"), points: trace, w: 0, h: 0 })
    setTrace([])
  }
  useEffect(() => { const k = (e: KeyboardEvent) => { if (e.key === 'Escape') { setTrace([]); setRegle([]); setSel(null) } if ((e.key === 'Delete' || e.key === 'Backspace') && sel && !(e.target as HTMLElement).closest('input,select,textarea')) ecrire(() => api.del(`/api/maps/${mid}/plan2d/${sel}`)).then(() => setSel(null)) }; window.addEventListener('keydown', k); return () => window.removeEventListener('keydown', k) }, [sel, mid])

  const etageObj = (o: Obj) => o.floor_id === etage
  const grille = useMemo(() => {
    // Pas de la grille : 1 m de près, 5 m ou 10 m de loin.
    const pas = zoom >= 12 ? 100 : zoom >= 4 ? 500 : 1000
    const [x0, y0] = monde(0, taille[1]), [x1, y1] = monde(taille[0], 0)
    const l: JSX.Element[] = []
    for (let x = Math.floor(x0 / pas) * pas; x <= x1; x += pas) { const [, sy] = ecran(x, 0); l.push(<line key={'x' + x} x1={0} x2={taille[0]} y1={sy} y2={sy} className={x === 0 ? 'p2-axe' : x % (pas * 10) === 0 ? 'p2-grille fort' : 'p2-grille'} />) }
    for (let y = Math.floor(y0 / pas) * pas; y <= y1; y += pas) { const [sx] = ecran(0, y); l.push(<line key={'y' + y} y1={0} y2={taille[1]} x1={sx} x2={sx} className={y === 0 ? 'p2-axe' : y % (pas * 10) === 0 ? 'p2-grille fort' : 'p2-grille'} />) }
    return { l, pas }
  }, [zoom, centre, taille])

  const dessin = (o: Obj, actif: boolean) => {
    const cls = 'p2-obj k-' + o.kind + (o.id === sel ? ' sel' : '') + (actif ? '' : ' fantome')
    const dx = glisse?.mode === 'move' && o.id === sel && curseur ? curseur[0] - glisse.from[0] : 0
    const dy = glisse?.mode === 'move' && o.id === sel && curseur ? curseur[1] - glisse.from[1] : 0
    if (o.points.length) {
      const pts = o.points.map(p => ecran(p[0], p[1]).join(',')).join(' ')
      return o.kind === 'polygone' ? <polygon key={o.id} data-obj={actif ? o.id : undefined} points={pts} className={cls} /> : <polyline key={o.id} data-obj={actif ? o.id : undefined} points={pts} className={cls} />
    }
    const [cx, cy] = ecran(o.x + dx, o.y + dy)
    const lw = o.h / 100 * zoom, lh = o.w / 100 * zoom // w le long de X (vertical), h le long de Y (horizontal)
    if (o.kind === 'cercle') return <g key={o.id}><circle data-obj={actif ? o.id : undefined} cx={cx} cy={cy} r={o.w / 200 * zoom} className={cls} />{actif && <text x={cx} y={cy} className="p2-nom">{o.name}</text>}</g>
    if (o.kind === 'appareil') return <g key={o.id} data-obj={actif ? o.id : undefined} className={cls}><rect x={cx - 7} y={cy - 7} width={14} height={14} rx={3} /><text x={cx} y={cy + 20} className="p2-nom">{o.name || o.device}</text></g>
    return <g key={o.id} transform={`rotate(${o.rotation} ${cx} ${cy})`}><rect data-obj={actif ? o.id : undefined} x={cx - lw / 2} y={cy - lh / 2} width={lw} height={lh} className={cls} />{actif && <text x={cx} y={cy} className="p2-nom">{o.name} · {m(o.w)} × {m(o.h)} m</text>}</g>
  }

  const dossiers = useMemo(() => [...new Set((niveau?.actors ?? []).map(a => a.folder || '').filter(Boolean))].sort(), [niveau])
  const etageCourant = data?.floors.find(f => f.id === etage)
  const acteurs = (niveau?.actors ?? []).filter(a => (!dossier || a.folder === dossier) && (!etageCourant || (a.z >= etageCourant.z_min - 1 && a.z < etageCourant.z_max)))

  if (!mid) return null
  return (
    <div className="stack p2-page" style={{ gap: 14 }}>
      <h1 className="forge-h1">{tr("Plan 2D")}</h1>
      <p className="muted" style={{ margin: 0 }}>{tr("Le plan de la map à l'échelle, étage par étage. 1 case = 1 m de près ; positions en cm comme le panneau Détails d'UEFN (X vers l'avant, Y vers la droite).")}</p>
      <section className="card p2">
        <div className="row p2-etages" role="tablist">
          {data?.floors.map(f => <button key={f.id} role="tab" aria-selected={f.id === etage} className={'btn' + (f.id === etage ? ' pri' : '')} onClick={() => { setEtage(f.id); setSel(null) }}>{f.name} <span className="small">{m(f.z_min)}–{m(f.z_max)} m</span></button>)}
          <button className="btn" onClick={() => { const n = prompt(tr("Nom du nouvel étage"), tr("Étage {0}", [data?.floors.length ?? 1])); if (n) ecrire(() => api.post(`/api/maps/${mid}/plan2d/floors`, { name: n })) }}><Plus size={16} strokeWidth={1.5} />{tr("Étage")}</button>
          <span style={{ flex: 1 }} />
          <label className="row small" style={{ gap: 6 }}><input type="checkbox" checked={niveauOn} onChange={e => setNiveauOn(e.target.checked)} /><Layers size={16} strokeWidth={1.5} />{tr("Niveau réel (UEFN)")}</label>
          {niveauOn && <select className="field" style={{ width: 'auto', height: 32 }} aria-label={tr("Dossier")} value={dossier} onChange={e => setDossier(e.target.value)}><option value="">{tr("Tous les dossiers")}</option>{dossiers.map(d => <option key={d} value={d}>{d}</option>)}</select>}
        </div>
        <div className="row p2-outils">
          {OUTILS.map(([k, l, I]) => <button key={k} className={'iconbtn' + (outil === k ? ' on' : '')} aria-pressed={outil === k} aria-label={l} data-tip={l} onClick={() => { setOutil(k); setTrace([]); setRegle([]) }}><I size={18} strokeWidth={1.5} /></button>)}
          {outil === 'appareil' && <select className="field" style={{ width: 'auto', height: 32 }} aria-label={tr("Appareil")} value={appareil} onChange={e => setAppareil(e.target.value)}>{APPAREILS.map(a => <option key={a[0]} value={a[0]}>{a[1]}</option>)}</select>}
          {(outil === 'polygone' || outil === 'chemin' || outil === 'mur') && <span className="small muted">{trace.length ? <button className="btn" onClick={finirTrace}>{tr("Terminer ({0} points)", [trace.length])}</button> : tr("Cliquez les points, puis « Terminer » ou double-clic.")}</span>}
          {outil === 'regle' && regle.length === 2 && <b className="p2-mesure">{tr("{0} m", [metres(Math.hypot(regle[1][0] - regle[0][0], regle[1][1] - regle[0][1]))])}</b>}
          <span style={{ flex: 1 }} />
          <span className="small muted mono">{curseur ? `X ${Math.round(curseur[0])} · Y ${Math.round(curseur[1])} cm` : ''}</span>
          <button className="iconbtn" aria-label={tr("Dézoomer")} onClick={() => setZoom(z => Math.max(0.5, z / 1.5))}>−</button>
          <span className="small mono">{tr("1 m = {0} px", [Math.round(zoom * 10) / 10])}</span>
          <button className="iconbtn" aria-label={tr("Zoomer")} onClick={() => setZoom(z => Math.min(80, z * 1.5))}>+</button>
        </div>
        <div className="p2-zone">
          <svg ref={svg} className="p2-svg" onMouseDown={down} onMouseMove={move} onMouseUp={up} onMouseLeave={() => { setCurseur(null); setGlisse(null) }} onDoubleClick={finirTrace}
            onWheel={e => setZoom(z => Math.min(80, Math.max(0.5, z * (e.deltaY < 0 ? 1.15 : 1 / 1.15))))} role="img" aria-label={tr("Plan 2D")}>
            {grille.l}
            {niveauOn && acteurs.map(a => { const [cx, cy] = ecran(a.x, a.y); const lw = a.h / 100 * zoom, lh = a.w / 100 * zoom; return <g key={a.id} className="p2-acteur"><rect x={cx - lw / 2} y={cy - lh / 2} width={Math.max(lw, 3)} height={Math.max(lh, 3)}><title>{`${a.name} · ${a.folder || ''} · ${m(a.w)} × ${m(a.h)} m`}</title></rect></g> })}
            {objets.filter(o => !etageObj(o)).map(o => dessin(o, false))}
            {objets.filter(etageObj).map(o => dessin(o, true))}
            {glisse?.mode === 'draw' && curseur && (() => { const a = glisse.from, b = arrondi(curseur); if (outil === 'cercle') { const [cx, cy] = ecran(a[0], a[1]); return <circle cx={cx} cy={cy} r={Math.hypot(b[0] - a[0], b[1] - a[1]) / 100 * zoom} className="p2-apercu" /> } const [x1, y1] = ecran(a[0], a[1]), [x2, y2] = ecran(b[0], b[1]); return <g><rect x={Math.min(x1, x2)} y={Math.min(y1, y2)} width={Math.abs(x2 - x1)} height={Math.abs(y2 - y1)} className="p2-apercu" /><text x={(x1 + x2) / 2} y={(y1 + y2) / 2} className="p2-nom">{m(Math.abs(b[0] - a[0]))} × {m(Math.abs(b[1] - a[1]))} m</text></g> })()}
            {trace.length > 0 && <polyline points={[...trace, ...(curseur ? [arrondi(curseur)] : [])].map(p => ecran(p[0], p[1]).join(',')).join(' ')} className="p2-apercu" />}
            {regle.length > 0 && (() => { const pts = [...regle, ...(regle.length === 1 && curseur ? [arrondi(curseur)] : [])]; const s = pts.map(p => ecran(p[0], p[1])); return <g className="p2-regle">{s.length === 2 && <line x1={s[0][0]} y1={s[0][1]} x2={s[1][0]} y2={s[1][1]} />}{s.map((p, i) => <circle key={i} cx={p[0]} cy={p[1]} r={4} />)}{s.length === 2 && <text x={(s[0][0] + s[1][0]) / 2} y={(s[0][1] + s[1][1]) / 2 - 8} className="p2-nom">{metres(Math.hypot(pts[1][0] - pts[0][0], pts[1][1] - pts[0][1]))} m</text>}</g> })()}
          </svg>
          {choisi && <aside className="p2-panneau">
            <label className="label">{tr("Nom")}<input className="field" defaultValue={choisi.name} key={'n' + choisi.id} onBlur={e => e.target.value !== choisi.name && maj(choisi.id, { name: e.target.value })} /></label>
            {choisi.kind === 'appareil' && <label className="label">{tr("Appareil")}<select className="field" value={choisi.device} onChange={e => maj(choisi.id, { device: e.target.value })}>{APPAREILS.map(a => <option key={a[0]} value={a[0]}>{a[1]}</option>)}</select></label>}
            {!choisi.points.length && <div className="grid2">
              <label className="label">X (cm)<input className="field" type="number" defaultValue={choisi.x} key={'x' + choisi.id + choisi.x} onBlur={e => maj(choisi.id, { x: Number(e.target.value) })} /></label>
              <label className="label">Y (cm)<input className="field" type="number" defaultValue={choisi.y} key={'y' + choisi.id + choisi.y} onBlur={e => maj(choisi.id, { y: Number(e.target.value) })} /></label>
              {choisi.kind !== 'appareil' && <>
                <label className="label">{choisi.kind === 'cercle' ? tr("Diamètre (m)") : tr("Long. X (m)")}<input className="field" type="number" step="0.5" defaultValue={m(choisi.w)} key={'w' + choisi.id + choisi.w} onBlur={e => maj(choisi.id, choisi.kind === 'cercle' ? { w: Number(e.target.value) * 100, h: Number(e.target.value) * 100 } : { w: Number(e.target.value) * 100 })} /></label>
                {choisi.kind !== 'cercle' && <label className="label">{tr("Larg. Y (m)")}<input className="field" type="number" step="0.5" defaultValue={m(choisi.h)} key={'h' + choisi.id + choisi.h} onBlur={e => maj(choisi.id, { h: Number(e.target.value) * 100 })} /></label>}
              </>}
            </div>}
            <label className="label">{tr("Étage")}<select className="field" value={choisi.floor_id} onChange={e => maj(choisi.id, { floor_id: Number(e.target.value) })}>{data?.floors.map(f => <option key={f.id} value={f.id}>{f.name}</option>)}</select></label>
            <button className="btn" onClick={() => ecrire(() => api.del(`/api/maps/${mid}/plan2d/${choisi.id}`)).then(() => setSel(null))}><Trash2 size={16} strokeWidth={1.5} />{tr("Supprimer")}</button>
          </aside>}
        </div>
        {niveauOn && niveau && niveau.actors.length === 0 && <p className="small muted" style={{ margin: '8px 0 0' }}>{tr("Aucun acteur reçu du niveau UEFN : reliez le projet et lancez une synchronisation.")}</p>}
      </section>
    </div>
  )
}
