// Mise en forme du tableau blanc : texte riche (gras, italique, souligné, barré, couleurs, listes, liens),
// police, taille, alignement, remplissage, bordure, opacité, flèches et stylo.
import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import {
  Bold, Italic, Underline, Strikethrough, AlignLeft, AlignCenter, AlignRight, List, ListOrdered, Link2, Highlighter,
  AlignVerticalJustifyStart, AlignVerticalJustifyCenter, AlignVerticalJustifyEnd, PaintBucket, Square, Lock, Unlock,
  Paintbrush, ClipboardPaste, Minus, Plus, MoveRight, MoveLeft, Baseline, Eraser, Shapes,
} from 'lucide-react'

export type O = Record<string, any>

// Palette proche de Miro : couleurs vives puis pastels.
export const COLORS = [
  '#1A1A1A', '#5F6368', '#A0A4AA', '#FFFFFF', '#E5484D', '#F76B15', '#FFB224', '#46A758', '#12A594', '#0090FF', '#3E63DD', '#8E4EC6', '#D6409F', '#8D5A3B',
  '#E9E9EB', '#F3F3F3', '#FFD8D6', '#FFE2CC', '#FFF3A3', '#D3F2D4', '#C9F0EA', '#CDE7FF', '#D9DEFA', '#EADCF8', '#FBD5EA', '#EBDDD3',
]
export const FONTS: Record<string, [string, string]> = {
  sans: ['Sans', "'Geist', system-ui, sans-serif"],
  serif: ['Serif', "Georgia, 'Times New Roman', serif"],
  mono: ['Mono', "'Geist Mono', ui-monospace, monospace"],
  hand: ['Manuscrite', "'Comic Sans MS', 'Chalkboard SE', 'Marker Felt', cursive"],
  round: ['Arrondie', "'Arial Rounded MT Bold', 'Nunito', 'Varela Round', sans-serif"],
}
export const SIZES = [10, 12, 14, 16, 18, 20, 24, 28, 32, 40, 48, 64, 80, 96]
export const SHAPES: [string, string][] = [['rect', 'Rectangle'], ['round', 'Arrondi'], ['ellipse', 'Cercle'], ['diamond', 'Losange'], ['triangle', 'Triangle'], ['hexagon', 'Hexagone'], ['para', 'Parallélogramme'], ['star', 'Étoile'], ['arrow', 'Flèche'], ['bubble', 'Bulle']]
const TEXT_KINDS = ['sticky', 'text', 'shape', 'comment', 'frame']
const STYLE_KEYS = ['fs', 'ff', 'ta', 'va', 'tc', 'bg', 'bc', 'bw', 'bs', 'op', 'b', 'i', 'u', 's', 'color', 'width', 'dashed', 'arrow', 'arrowStart', 'style']

// ---------- Texte riche ----------
const TAGS = new Set(['B', 'STRONG', 'I', 'EM', 'U', 'S', 'STRIKE', 'DEL', 'BR', 'DIV', 'P', 'SPAN', 'FONT', 'UL', 'OL', 'LI', 'A', 'MARK', 'SUB', 'SUP'])
const CSS_OK = /^(color|background-color|font-weight|font-style|text-decoration|text-decoration-line)$/
const VAL_OK = /^[#a-zA-Z0-9(),.%\s-]{1,60}$/
export function sanitize(html: string): string {
  if (!html) return ''
  const doc = new DOMParser().parseFromString(`<div>${html}</div>`, 'text/html')
  const walk = (n: Node): string => {
    if (n.nodeType === 3) return esc(n.textContent ?? '')
    if (n.nodeType !== 1) return ''
    const el = n as HTMLElement
    const inner = [...el.childNodes].map(walk).join('')
    if (!TAGS.has(el.tagName)) return inner
    let tag = el.tagName.toLowerCase()
    const attrs: string[] = []
    const st: string[] = []
    if (tag === 'font') { tag = 'span'; const c = el.getAttribute('color'); if (c && VAL_OK.test(c)) st.push(`color:${c}`) }
    el.style && [...el.style].forEach(p => { const v = el.style.getPropertyValue(p); if (CSS_OK.test(p) && VAL_OK.test(v)) st.push(`${p}:${v}`) })
    if (st.length) attrs.push(`style="${st.join(';')}"`)
    if (tag === 'a') { const h = el.getAttribute('href') ?? ''; if (/^(https?:\/\/|mailto:)/i.test(h)) attrs.push(`href="${esc(h)}" target="_blank" rel="noopener noreferrer"`) }
    if (tag === 'br') return '<br>'
    return `<${tag}${attrs.length ? ' ' + attrs.join(' ') : ''}>${inner}</${tag}>`
  }
  return [...doc.body.firstChild!.childNodes].map(walk).join('').slice(0, 20000)
}
const esc = (s: string) => s.replace(/[<>&"]/g, c => ({ '<': '&lt;', '>': '&gt;', '&': '&amp;', '"': '&quot;' } as any)[c])
export const textToHtml = (t: string) => esc(t ?? '').replace(/\n/g, '<br>')

export function textStyle(o: O): React.CSSProperties {
  const deco = [o.u && 'underline', o.s && 'line-through'].filter(Boolean).join(' ')
  return {
    fontSize: o.fs, fontFamily: o.ff ? FONTS[o.ff]?.[1] : undefined, textAlign: o.ta, color: o.tc,
    fontWeight: o.b ? 700 : undefined, fontStyle: o.i ? 'italic' : undefined, textDecoration: deco || undefined,
  }
}
export const vAlign = (o: O, def = 'flex-start') => (o.va === 'middle' ? 'center' : o.va === 'bottom' ? 'flex-end' : o.va === 'top' ? 'flex-start' : def)

/** Affiche le texte d'un objet (riche si disponible). */
export function Rich({ o, placeholder }: { o: O; placeholder?: string }) {
  if (o.html) return <div className="rt" dangerouslySetInnerHTML={{ __html: sanitize(o.html) }} />
  if (!o.text && placeholder) return <div className="rt" style={{ opacity: 0.4 }}>{placeholder}</div>
  return <div className="rt">{o.text}</div>
}

// Éditeur actif : on garde la sélection pour pouvoir appliquer une couleur après un clic dans la barre.
const active: { el: HTMLDivElement | null; range: Range | null; done?: () => void } = { el: null, range: null }
export const isEditing = () => !!active.el
function restore() {
  const el = active.el; if (!el) return false
  el.focus()
  if (active.range) { const s = window.getSelection(); s?.removeAllRanges(); s?.addRange(active.range) }
  return true
}
export function exec(cmd: string, val?: string) {
  if (!restore()) return false
  document.execCommand('styleWithCSS', false, (cmd === 'foreColor' || cmd === 'hiliteColor') as any)
  document.execCommand(cmd, false, val)
  const s = window.getSelection(); if (s && s.rangeCount) active.range = s.getRangeAt(0).cloneRange()
  active.done?.()
  return true
}

/** Zone d'édition riche (double-clic sur un objet). */
export function RichEdit({ o, style, onType, onDone }: { o: O; style?: React.CSSProperties; onType: (html: string, text: string) => void; onDone: (html: string, text: string) => void }) {
  const ref = useRef<HTMLDivElement>(null)
  const fin = useRef({ done: false, onDone })
  fin.current.onDone = onDone
  useEffect(() => {
    const el = ref.current!; el.innerHTML = o.html ? sanitize(o.html) : textToHtml(o.text ?? '')
    el.focus()
    const r = document.createRange(); r.selectNodeContents(el); if (o.text || o.html) r.collapse(false)
    const s = window.getSelection(); s?.removeAllRanges(); s?.addRange(r)
    active.el = el; active.range = r.cloneRange()
    active.done = () => onType(sanitize(el.innerHTML), el.innerText)
    const sc = () => { const s = window.getSelection(); if (s && s.rangeCount && el.contains(s.anchorNode)) active.range = s.getRangeAt(0).cloneRange() }
    document.addEventListener('selectionchange', sc)
    return () => {
      document.removeEventListener('selectionchange', sc)
      if (active.el === el) { active.el = null; active.range = null; active.done = undefined }
      if (!fin.current.done) { fin.current.done = true; fin.current.onDone(sanitize(el.innerHTML), el.innerText.replace(/\n$/, '')) }
    }
  }, [])
  const finish = () => { if (fin.current.done) return; fin.current.done = true; const el = ref.current!; onDone(sanitize(el.innerHTML), el.innerText.replace(/\n$/, '')) }
  return <div ref={ref} className="edit-in rt" contentEditable suppressContentEditableWarning style={style}
    onPointerDown={e => e.stopPropagation()}
    onInput={e => onType(sanitize(e.currentTarget.innerHTML), e.currentTarget.innerText)}
    onBlur={e => { if ((e.relatedTarget as HTMLElement | null)?.closest('.fmtbar')) return; finish() }}
    onPaste={e => { e.preventDefault(); const h = e.clipboardData.getData('text/html'); const t = e.clipboardData.getData('text/plain'); document.execCommand('insertHTML', false, h ? sanitize(h) : textToHtml(t)) }}
    onKeyDown={e => {
      e.stopPropagation()
      const mod = e.metaKey || e.ctrlKey
      if (e.key === 'Escape' || (e.key === 'Enter' && mod)) { e.preventDefault(); (e.currentTarget as HTMLDivElement).blur() }
      if (mod && e.key.toLowerCase() === 'k') { e.preventDefault(); askLink() }
      if (mod && e.shiftKey && e.key.toLowerCase() === 'x') { e.preventDefault(); exec('strikeThrough') }
      if (mod && e.shiftKey && (e.key === '7' || e.key === '&')) { e.preventDefault(); exec('insertOrderedList') }
      if (mod && e.shiftKey && (e.key === '8' || e.key === '_')) { e.preventDefault(); exec('insertUnorderedList') }
    }} />
}
function askLink() {
  const u = prompt('Adresse du lien (https://…) — laisser vide pour retirer')
  if (u === null) return
  if (!u) exec('unlink'); else exec('createLink', /^(https?:\/\/|mailto:)/i.test(u) ? u : 'https://' + u)
}

// ---------- Formes ----------
export function ShapeSvg({ o, sel }: { o: O; sel?: boolean }) {
  const fill = !o.bg || o.bg === 'none' ? 'transparent' : o.bg
  const stroke = o.bw === 0 ? 'transparent' : o.bc ?? 'var(--text)'
  const sw = o.bw ?? 1.5
  const dash = (o.bs ?? (o.dashed ? 'dashed' : 'solid')) === 'dashed' ? '8 6' : o.bs === 'dotted' ? '2 4' : undefined
  const p = { fill, stroke, strokeWidth: sw, strokeDasharray: dash, vectorEffect: 'non-scaling-stroke' as const, strokeLinejoin: 'round' as const }
  const s = o.shape ?? 'rect'
  const el = s === 'ellipse' ? <ellipse cx="50" cy="50" rx="49" ry="49" {...p} />
    : s === 'round' ? <rect x="1" y="1" width="98" height="98" rx="16" {...p} />
    : s === 'diamond' ? <polygon points="50,1 99,50 50,99 1,50" {...p} />
    : s === 'triangle' ? <polygon points="50,1 99,99 1,99" {...p} />
    : s === 'hexagon' ? <polygon points="25,1 75,1 99,50 75,99 25,99 1,50" {...p} />
    : s === 'para' ? <polygon points="20,1 99,1 80,99 1,99" {...p} />
    : s === 'star' ? <polygon points="50,2 61,37 98,37 68,59 79,96 50,74 21,96 32,59 2,37 39,37" {...p} />
    : s === 'arrow' ? <polygon points="1,30 65,30 65,4 99,50 65,96 65,70 1,70" {...p} />
    : s === 'bubble' ? <path d="M8 1 H92 Q99 1 99 8 V68 Q99 75 92 75 H40 L20 98 L24 75 H8 Q1 75 1 68 V8 Q1 1 8 1 Z" {...p} />
    : <rect x="1" y="1" width="98" height="98" rx="4" {...p} />
  return <svg className="shapebg" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden style={{ opacity: sel ? 1 : undefined }}>{el}</svg>
}

// ---------- Couleurs ----------
function Swatches({ value, onPick, none, label }: { value?: string; onPick: (c: string) => void; none?: string; label: string }) {
  return <div className="swpop" role="group" aria-label={label}>
    <div className="swgrid">
      {COLORS.map(c => <button key={c} className={'sw' + (value?.toLowerCase() === c.toLowerCase() ? ' on' : '')} style={{ background: c }} title={c} aria-label={c} onMouseDown={e => e.preventDefault()} onClick={() => onPick(c)} />)}
    </div>
    <div className="row" style={{ gap: 8, marginTop: 8 }}>
      {none && <button className="pill" onMouseDown={e => e.preventDefault()} onClick={() => onPick('none')}>{none}</button>}
      <label className="pill" title="Couleur personnalisée"><span className="sw" style={{ width: 14, height: 14, background: 'conic-gradient(red,yellow,lime,cyan,blue,magenta,red)' }} />Autre<input type="color" value={value && value.startsWith('#') && value.length === 7 ? value : '#000000'} onChange={e => onPick(e.target.value)} style={{ position: 'absolute', opacity: 0, width: 0, height: 0 }} /></label>
    </div>
  </div>
}

// ---------- Barre de mise en forme ----------
type Props = {
  sel: O[]; editing: boolean; push: (list: O[]) => void
  pos: { left: number; top: number; bottom: number } | null; extra?: React.ReactNode
}
let styleClip: O | null = null

export function FormatBar({ sel, editing, push, pos, extra }: Props) {
  const [pop, setPop] = useState<string | null>(null)
  const bar = useRef<HTMLDivElement>(null)
  useEffect(() => setPop(null), [sel.map(o => o.id).join(), editing])
  // Garde la barre dans le tableau, au-dessus de la sélection (ou en dessous s'il n'y a pas la place).
  useLayoutEffect(() => {
    const el = bar.current, par = el?.parentElement; if (!el || !par || !pos) return
    const w = el.offsetWidth, h = el.offsetHeight, W = par.clientWidth, H = par.clientHeight
    el.style.left = Math.min(W - w / 2 - 8, Math.max(w / 2 + 8, pos.left)) + 'px'
    const above = pos.top - h - 10
    el.style.top = (above >= 8 ? above : Math.min(H - h - 8, pos.bottom + 12)) + 'px'
  })
  if (!sel.length) return null
  const first = sel[0]
  const kinds = new Set(sel.map(o => o.kind))
  const hasText = sel.some(o => TEXT_KINDS.includes(o.kind))
  const hasFill = sel.some(o => ['sticky', 'shape', 'text', 'comment', 'frame'].includes(o.kind))
  const hasBorder = sel.some(o => ['shape', 'text', 'frame', 'comment'].includes(o.kind))
  const onlyLines = [...kinds].every(k => k === 'connector' || k === 'stroke')
  const isConn = kinds.has('connector')
  const isShape = kinds.has('shape')
  const set = (patch: O | ((o: O) => O), filter?: (o: O) => boolean) => {
    const list = sel.filter(o => !filter || filter(o)).map(o => ({ ...o, ...(typeof patch === 'function' ? patch(o) : patch) }))
    if (list.length) push(list)
  }
  const txt = (o: O) => TEXT_KINDS.includes(o.kind)
  const all = (k: string) => sel.filter(txt).every(o => o[k])
  const toggle = (k: 'b' | 'i' | 'u' | 's') => {
    const cmd = { b: 'bold', i: 'italic', u: 'underline', s: 'strikeThrough' }[k]
    if (editing) exec(cmd); else set({ [k]: !all(k) }, txt)
  }
  const size = first.fs ?? (first.kind === 'text' ? 18 : first.kind === 'shape' ? 13 : 14)
  const bump = (d: number) => { const i = SIZES.findIndex(s => s >= size); const n = SIZES[Math.min(SIZES.length - 1, Math.max(0, (SIZES[i] === size ? i : d > 0 ? i - 1 : i) + d))] ?? size; set({ fs: n }, txt) }
  const B = ({ t, on, onClick, children, k }: { t: string; on?: boolean; onClick: () => void; children: React.ReactNode; k?: string }) =>
    <button key={k} className={'fbtn' + (on ? ' on' : '')} title={t} aria-label={t} aria-pressed={on} onMouseDown={e => e.preventDefault()} onClick={onClick}>{children}</button>
  const P = (id: string, t: string, face: React.ReactNode, body: React.ReactNode) => <div className="fpop">
    <button className={'fbtn' + (pop === id ? ' on' : '')} title={t} aria-label={t} aria-expanded={pop === id} onMouseDown={e => e.preventDefault()} onClick={() => setPop(pop === id ? null : id)}>{face}</button>
    {pop === id && <div className="fmenu" onMouseDown={e => { if ((e.target as HTMLElement).tagName !== 'INPUT') e.preventDefault() }}>{body}</div>}
  </div>
  const Sep = () => <span className="fsep" />

  return <div ref={bar} className="fmtbar" style={pos ? undefined : { left: '50%', bottom: 16 }} onPointerDown={e => e.stopPropagation()} onDoubleClick={e => e.stopPropagation()}>
    {isShape && !editing && P('shape', 'Type de forme', <Shapes size={16} />, <div className="fgrid">
      {SHAPES.map(([k, l]) => <button key={k} className={'shapepick' + (first.shape === k ? ' on' : '')} title={l} aria-label={l} onClick={() => set({ shape: k }, o => o.kind === 'shape')}><svg viewBox="0 0 100 100" width="26" height="26"><ShapeIcon k={k} /></svg></button>)}
    </div>)}

    {hasText && <>
      {P('font', 'Police', <span className="fface">{FONTS[first.ff ?? 'sans'][0]}</span>, <div className="flist">
        {Object.entries(FONTS).map(([k, [l, f]]) => <button key={k} className={(first.ff ?? 'sans') === k ? 'on' : ''} style={{ fontFamily: f }} onClick={() => { set({ ff: k }, txt); setPop(null) }}>{l}</button>)}
      </div>)}
      <div className="fsize">
        <B t="Réduire la taille" onClick={() => bump(-1)}><Minus size={13} /></B>
        {P('size', 'Taille du texte', <span className="mono">{size}</span>, <div className="flist" style={{ maxHeight: 260, overflow: 'auto' }}>
          {SIZES.map(s => <button key={s} className={s === size ? 'on' : ''} onClick={() => { set({ fs: s }, txt); setPop(null) }}>{s}</button>)}
        </div>)}
        <B t="Agrandir la taille" onClick={() => bump(1)}><Plus size={13} /></B>
      </div>
      <Sep />
      <B t="Gras (Ctrl+B)" on={!editing && all('b')} onClick={() => toggle('b')}><Bold size={16} /></B>
      <B t="Italique (Ctrl+I)" on={!editing && all('i')} onClick={() => toggle('i')}><Italic size={16} /></B>
      <B t="Souligné (Ctrl+U)" on={!editing && all('u')} onClick={() => toggle('u')}><Underline size={16} /></B>
      <B t="Barré (Ctrl+Maj+X)" on={!editing && all('s')} onClick={() => toggle('s')}><Strikethrough size={16} /></B>
      {P('tc', 'Couleur du texte', <span className="fcol"><Baseline size={16} /><i style={{ background: first.tc ?? 'var(--text)' }} /></span>,
        <Swatches label="Couleur du texte" value={first.tc} none="Par défaut" onPick={c => { if (editing) exec('foreColor', c === 'none' ? 'inherit' : c); else set(o => ({ tc: c === 'none' ? undefined : c, html: o.html ? stripColor(o.html) : o.html }), txt); setPop(null) }} />)}
      {editing && P('hl', 'Surligner', <Highlighter size={16} />, <Swatches label="Surlignage" none="Aucun" onPick={c => { exec('hiliteColor', c === 'none' ? 'transparent' : c); setPop(null) }} />)}
      <Sep />
      {P('ta', 'Alignement', first.ta === 'center' ? <AlignCenter size={16} /> : first.ta === 'right' ? <AlignRight size={16} /> : <AlignLeft size={16} />, <div className="frow">
        <B t="À gauche" on={(first.ta ?? 'left') === 'left'} onClick={() => set({ ta: 'left' }, txt)}><AlignLeft size={16} /></B>
        <B t="Centré" on={first.ta === 'center'} onClick={() => set({ ta: 'center' }, txt)}><AlignCenter size={16} /></B>
        <B t="À droite" on={first.ta === 'right'} onClick={() => set({ ta: 'right' }, txt)}><AlignRight size={16} /></B>
        <span className="fsep" />
        <B t="En haut" on={first.va === 'top'} onClick={() => set({ va: 'top' }, txt)}><AlignVerticalJustifyStart size={16} /></B>
        <B t="Au milieu" on={first.va === 'middle'} onClick={() => set({ va: 'middle' }, txt)}><AlignVerticalJustifyCenter size={16} /></B>
        <B t="En bas" on={first.va === 'bottom'} onClick={() => set({ va: 'bottom' }, txt)}><AlignVerticalJustifyEnd size={16} /></B>
      </div>)}
      {editing && <>
        <B t="Liste à puces (Ctrl+Maj+8)" onClick={() => exec('insertUnorderedList')}><List size={16} /></B>
        <B t="Liste numérotée (Ctrl+Maj+7)" onClick={() => exec('insertOrderedList')}><ListOrdered size={16} /></B>
        <B t="Lien (Ctrl+K)" onClick={askLink}><Link2 size={16} /></B>
        <B t="Effacer la mise en forme" onClick={() => exec('removeFormat')}><Eraser size={16} /></B>
      </>}
    </>}

    {!editing && hasFill && <>
      <Sep />
      {P('bg', 'Couleur de fond', <span className="fcol"><PaintBucket size={16} /><i style={{ background: first.bg && first.bg !== 'none' ? first.bg : 'transparent', border: '1px solid var(--border)' }} /></span>,
        <Swatches label="Couleur de fond" value={first.bg} none="Sans fond" onPick={c => { set(o => o.kind === 'sticky' && c === 'none' ? { bg: undefined } : { bg: c, fill: undefined }, o => o.kind !== 'connector' && o.kind !== 'stroke'); setPop(null) }} />)}
    </>}
    {!editing && hasBorder && P('bd', 'Bordure', <Square size={16} style={{ color: first.bc }} />, <div style={{ width: 252 }}>
      <Swatches label="Couleur de bordure" value={first.bc} none="Sans bordure" onPick={c => set(c === 'none' ? { bw: 0 } : { bc: c, bw: first.bw || 1.5 })} />
      <div className="frow" style={{ marginTop: 10 }}>
        <span className="small muted" style={{ width: 60 }}>Épaisseur</span>
        {[0, 1, 2, 4, 6].map(w => <B key={w} t={`${w} px`} on={(first.bw ?? (first.kind === 'shape' ? 1.5 : 0)) === w || (w === 1 && first.bw === undefined && first.kind === 'shape')} onClick={() => set({ bw: w })}><span style={{ width: 16, height: Math.max(1, w), background: w ? 'currentColor' : 'transparent', borderTop: w ? 0 : '1px dashed currentColor' }} /></B>)}
      </div>
      <div className="frow">
        <span className="small muted" style={{ width: 60 }}>Trait</span>
        {[['solid', 'Plein'], ['dashed', 'Tirets'], ['dotted', 'Points']].map(([k, l]) => <B key={k} t={l} on={(first.bs ?? (first.dashed ? 'dashed' : 'solid')) === k} onClick={() => set({ bs: k, dashed: undefined, bw: first.bw || 1.5 })}><span style={{ width: 18, borderTop: `2px ${k} currentColor` }} /></B>)}
      </div>
    </div>)}

    {!editing && onlyLines && <>
      {P('lc', 'Couleur du trait', <span className="fcol"><Paintbrush size={16} /><i style={{ background: first.color?.startsWith?.('#') ? first.color : 'var(--text)' }} /></span>,
        <Swatches label="Couleur du trait" value={first.color} onPick={c => { set({ color: c }); setPop(null) }} />)}
      {P('lw', 'Épaisseur', <span className="mono small">{first.width ?? (isConn ? 1.6 : 2.5)}</span>, <div className="flist">
        {[1, 2, 3, 4, 6, 8, 12, 16].map(w => <button key={w} onClick={() => { set({ width: w }); setPop(null) }}><span style={{ display: 'inline-block', width: 40, height: Math.min(w, 10), background: 'currentColor', borderRadius: 4, marginRight: 8, verticalAlign: 'middle' }} />{w}</button>)}
      </div>)}
      <B t="Pointillés" on={sel.every(o => o.dashed)} onClick={() => set({ dashed: !sel.every(o => o.dashed) })}><span style={{ width: 18, borderTop: '2px dashed currentColor' }} /></B>
      {isConn && <>
        {P('cs', 'Forme de la ligne', <span className="small">{({ straight: 'Droit', elbow: 'Coudé', curve: 'Courbe' } as any)[first.style ?? 'curve']}</span>, <div className="flist">
          {[['straight', 'Droit'], ['elbow', 'Coudé'], ['curve', 'Courbe']].map(([k, l]) => <button key={k} onClick={() => { set({ style: k }, o => o.kind === 'connector'); setPop(null) }}>{l}</button>)}
        </div>)}
        <B t="Flèche au départ" on={sel.every(o => o.arrowStart)} onClick={() => set({ arrowStart: !sel.every(o => o.arrowStart) }, o => o.kind === 'connector')}><MoveLeft size={16} /></B>
        <B t="Flèche à l’arrivée" on={sel.every(o => o.arrow)} onClick={() => set({ arrow: !sel.every(o => o.arrow) }, o => o.kind === 'connector')}><MoveRight size={16} /></B>
      </>}
    </>}

    {!editing && <>
      <Sep />
      {P('op', 'Opacité', <span className="mono small">{Math.round((first.op ?? 1) * 100)}%</span>, <div style={{ width: 200, padding: 4 }}>
        <input type="range" min={10} max={100} step={5} value={Math.round((first.op ?? 1) * 100)} onChange={e => set({ op: Number(e.target.value) / 100 })} style={{ width: '100%' }} aria-label="Opacité" />
      </div>)}
      <B t="Copier le style" onClick={() => { styleClip = Object.fromEntries(STYLE_KEYS.filter(k => first[k] !== undefined).map(k => [k, first[k]])) }}><Paintbrush size={16} /></B>
      <B t="Coller le style" onClick={() => { if (styleClip) set(o => Object.fromEntries(Object.entries(styleClip!).filter(([k]) => (o.kind === 'connector' || o.kind === 'stroke') === ['color', 'width', 'dashed', 'arrow', 'arrowStart', 'style'].includes(k) || k === 'op'))) }}><ClipboardPaste size={16} /></B>
      <B t={sel.every(o => o.lk) ? 'Déverrouiller' : 'Verrouiller (empêche de déplacer)'} on={sel.every(o => o.lk)} onClick={() => set({ lk: !sel.every(o => o.lk) })}>{sel.every(o => o.lk) ? <Lock size={16} /> : <Unlock size={16} />}</B>
    </>}
    {extra && !editing && <><Sep />{extra}</>}
  </div>
}
const stripColor = (h: string) => h.replace(/color:[^;"]+;?/g, '')
function ShapeIcon({ k }: { k: string }) {
  const p = { fill: 'none', stroke: 'currentColor', strokeWidth: 6 }
  return <ShapeSvgInner k={k} p={p} />
}
function ShapeSvgInner({ k, p }: { k: string; p: any }) {
  if (k === 'ellipse') return <ellipse cx="50" cy="50" rx="44" ry="44" {...p} />
  if (k === 'round') return <rect x="6" y="14" width="88" height="72" rx="18" {...p} />
  if (k === 'diamond') return <polygon points="50,5 95,50 50,95 5,50" {...p} />
  if (k === 'triangle') return <polygon points="50,8 94,92 6,92" {...p} />
  if (k === 'hexagon') return <polygon points="27,8 73,8 95,50 73,92 27,92 5,50" {...p} />
  if (k === 'para') return <polygon points="24,14 95,14 76,86 5,86" {...p} />
  if (k === 'star') return <polygon points="50,6 61,38 95,38 67,58 78,92 50,72 22,92 33,58 5,38 39,38" {...p} />
  if (k === 'arrow') return <polygon points="5,34 62,34 62,12 95,50 62,88 62,66 5,66" {...p} />
  if (k === 'bubble') return <path d="M12 8 H88 Q94 8 94 14 V62 Q94 68 88 68 H42 L22 92 L26 68 H12 Q6 68 6 62 V14 Q6 8 12 8 Z" {...p} />
  return <rect x="6" y="14" width="88" height="72" rx="4" {...p} />
}

/** Barre du stylo : couleur et épaisseur avant de dessiner. */
export function PenBar({ color, width, onColor, onWidth }: { color: string; width: number; onColor: (c: string) => void; onWidth: (w: number) => void }) {
  return <div className="row" style={{ gap: 4 }}>
    {['#1A1A1A', '#E5484D', '#F76B15', '#FFB224', '#46A758', '#0090FF', '#8E4EC6', '#FFFFFF'].map(c => <button key={c} className={'sw' + (color === c ? ' on' : '')} style={{ background: c, width: 20, height: 20 }} aria-label={c} title={c} onClick={() => onColor(c)} />)}
    <label className="sw" title="Autre couleur" style={{ width: 20, height: 20, background: 'conic-gradient(red,yellow,lime,cyan,blue,magenta,red)', position: 'relative' }}><input type="color" value={color} onChange={e => onColor(e.target.value)} style={{ position: 'absolute', opacity: 0, width: 0, height: 0 }} /></label>
    <span className="tsep" />
    {[2, 4, 8, 14].map(w => <button key={w} className={'tool' + (width === w ? ' on' : '')} style={{ width: 30 }} title={`${w} px`} aria-label={`Épaisseur ${w}`} onClick={() => onWidth(w)}><span style={{ width: Math.min(w + 2, 16), height: Math.min(w + 2, 16), borderRadius: '50%', background: 'currentColor' }} /></button>)}
  </div>
}

/** Couleur effective d'un trait. */
export const strokeColor = (o: O, dark = 'var(--text)') => o.color && o.color.startsWith('#') ? o.color : dark
