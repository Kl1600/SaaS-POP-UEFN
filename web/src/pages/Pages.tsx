import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { useParams } from 'react-router-dom'
import {
  GripVertical, Plus, Star, FileText, Copy, Trash2, Type, Heading1, Heading2, Heading3, List, ListOrdered, CheckSquare, ChevronRight, ChevronDown, MoreHorizontal,
  AlertCircle, Quote, Code, Table, Image, Video, Paperclip, Columns2, ListTree, Link2, PenTool, Database, Minus, Download, History, Share2, MessageSquare, RotateCcw, X,
} from 'lucide-react'
import { ago, api, frDate } from '../api'
import { useApp, useLive, useLoad } from '../state'
import { Avatar, CardHead, Empty, PageHead, Tabs } from '../components/ui'
import { tr } from '../i18n/t'
import { useNav } from '../routes'

type Block = { id: string; type: string; text?: string; checked?: boolean; open?: boolean; body?: string; lang?: string; rows?: string[][]; url?: string; cols?: string[]; page?: number; board?: number; title?: string; view?: string; columns?: { key: string; label: string; type: string }[]; dbrows?: any[] } & Record<string, any>

const uid = () => Math.random().toString(36).slice(2, 12)
const TEXT_TYPES = ['p', 'h1', 'h2', 'h3', 'bullet', 'number', 'todo', 'toggle', 'callout', 'quote', 'code']

const INSERT: [string, string, string, any][] = [
  ['p', tr("Texte"), tr("Paragraphe"), Type], ['h1', tr("Titre 1"), tr("Grande section"), Heading1], ['h2', tr("Titre 2"), tr("Section"), Heading2], ['h3', tr("Titre 3"), tr("Sous-section"), Heading3],
  ['bullet', tr("Liste à puces"), tr("Liste simple"), List], ['number', tr("Liste numérotée"), '1, 2, 3', ListOrdered], ['todo', tr("Case à cocher"), tr("Liste de tâches"), CheckSquare],
  ['toggle', tr("Bloc repliable"), tr("Contenu masqué"), ChevronRight], ['callout', tr("Encadré"), tr("Mettre en avant"), AlertCircle], ['quote', tr("Citation"), tr("Citer un passage"), Quote],
  ['code', tr("Code (Verse)"), tr("Bloc de code"), Code], ['table', tr("Tableau"), tr("Lignes et colonnes"), Table], ['image', tr("Image"), tr("Par lien"), Image], ['video', tr("Vidéo"), tr("YouTube ou lien"), Video],
  ['file', tr("Fichier"), tr("Lien vers un fichier"), Paperclip], ['columns', tr("Colonnes"), tr("2 colonnes de texte"), Columns2], ['toc', tr("Sommaire"), tr("Titres de la page"), ListTree],
  ['pagelink', tr("Lien vers une page"), tr("Une autre page"), Link2], ['board', tr("Tableau blanc"), tr("Intégrer un tableau"), PenTool], ['database', tr("Base de données"), tr("Tableau, Kanban, Calendrier…"), Database],
  ['divider', tr("Séparateur"), tr("Ligne horizontale"), Minus],
]

function newBlock(type: string): Block {
  const b: Block = { id: uid(), type, text: '' }
  if (type === 'table') b.rows = [['', '', ''], ['', '', '']]
  if (type === 'code') b.lang = 'verse'
  if (type === 'columns') b.cols = ['', '']
  if (type === 'database') Object.assign(b, { title: tr("Nouvelle base"), view: 'table', columns: [{ key: 'name', label: tr("Nom"), type: 'text' }, { key: 'status', label: tr("Statut"), type: 'select' }, { key: 'date', label: tr("Date"), type: 'date' }, { key: 'owner', label: tr("Personne"), type: 'person' }], rows: [] })
  return b
}

/** Champ de texte éditable sans perdre le curseur quand l'état se met à jour. */
function Editable({ value, onChange, onKeyDown, className, ph, focusKey, disabled, onFocus }: {
  value: string; onChange: (v: string) => void; onKeyDown?: (e: React.KeyboardEvent<HTMLDivElement>) => void; className?: string; ph?: string; focusKey?: string; disabled?: boolean; onFocus?: () => void
}) {
  const ref = useRef<HTMLDivElement>(null)
  useLayoutEffect(() => { if (ref.current && ref.current.innerText !== value) ref.current.innerText = value }, [value])
  useEffect(() => {
    if (focusKey && ref.current) {
      ref.current.focus()
      const r = document.createRange(); r.selectNodeContents(ref.current); r.collapse(false)
      const s = window.getSelection(); s?.removeAllRanges(); s?.addRange(r)
    }
  }, [focusKey])
  return <div ref={ref} className={'btext ' + (className ?? '')} contentEditable={!disabled} suppressContentEditableWarning data-ph={ph}
    onInput={e => onChange((e.target as HTMLDivElement).innerText)} onKeyDown={onKeyDown} onFocus={onFocus}
    onPaste={e => { e.preventDefault(); document.execCommand('insertText', false, e.clipboardData.getData('text/plain')) }} />
}

export function BlockView({ b }: { b: Block }) {
  const t = b.text ?? ''
  switch (b.type) {
    case 'h1': return <h1>{t}</h1>
    case 'h2': return <h2>{t}</h2>
    case 'h3': return <h3>{t}</h3>
    case 'bullet': return <p>• {t}</p>
    case 'todo': return <p>{b.checked ? '☑' : '☐'} {t}</p>
    case 'quote': case 'callout': return <blockquote className="b-callout">{t}</blockquote>
    case 'code': return <pre className="b-code">{t}</pre>
    case 'divider': return <hr />
    case 'table': return <table className="tbl"><tbody>{b.rows?.map((r, i) => <tr key={i}>{r.map((c, j) => <td key={j}>{c}</td>)}</tr>)}</tbody></table>
    case 'toggle': return <details><summary>{t}</summary><p>{b.body}</p></details>
    default: return t ? <p>{t}</p> : null
  }
}

export default function Pages() {
  const { pid } = useParams()
  const nav = useNav()
  const { map, canEdit, toast, role } = useApp()
  const canDelete = ['owner', 'admin', 'member'].includes(role)
  const [menu, setMenu] = useState<number | null>(null)
  const { data: tree, reload: reloadTree } = useLoad<any[]>(map ? `/api/maps/${map.id}/pages` : null, [map?.id])
  const { data: templates } = useLoad<any[]>('/api/page-templates')
  const [tplOpen, setTplOpen] = useState(false)
  useLive(['pages'], reloadTree)
  const id = pid ? Number(pid) : tree?.find(p => p.favorite)?.id ?? tree?.[0]?.id

  const create = async (template?: string, parent_id?: number) => {
    try { const r = await api.post(`/api/maps/${map!.id}/pages`, { template, parent_id }); setTplOpen(false); await reloadTree(); nav(`/pages/${r.id}`) } catch (e: any) { toast(e.message, true) }
  }
  const removePage = async (p: any) => {
    const n = (tree ?? []).filter(x => x.parent_id === p.id).length
    if (!confirm(tr("Supprimer « {0} »{1} ? Elle ira dans la corbeille (restaurable).", [p.title || tr("Sans titre"), n ? tr(" et ses {0} sous-page(s)", [n]) : '']))) return
    try { await api.del(`/api/pages/${p.id}`); setMenu(null); await reloadTree(); if (p.id === id) nav('/pages'); toast(tr("Page mise à la corbeille")) } catch (e: any) { toast(e.message, true) }
  }
  const renamePage = async (p: any) => {
    const t = prompt(tr("Nouveau titre"), p.title); if (!t) return
    try { await api.patch(`/api/pages/${p.id}`, { title: t }); setMenu(null); reloadTree() } catch (e: any) { toast(e.message, true) }
  }
  const children = (p: number | null) => (tree ?? []).filter(x => (x.parent_id ?? null) === p)
  const Node = ({ p, depth }: { p: any; depth: number }) => {
    const [open, setOpen] = useState(true)
    const kids = children(p.id)
    return <>
      <div className="row tnode" style={{ gap: 0, position: 'relative' }}>
        <button className="titem" style={{ width: 24, padding: 0, justifyContent: 'center', marginLeft: depth * 14 }} aria-label={open ? tr("Replier") : tr("Déplier")} onClick={() => setOpen(!open)}>{kids.length ? (open ? <ChevronDown size={14} /> : <ChevronRight size={14} />) : null}</button>
        <button className={'titem' + (p.id === id ? ' on' : '')} onClick={() => nav(`/pages/${p.id}`)} onContextMenu={e => { e.preventDefault(); setMenu(p.id) }}><span className="pico">{p.icon || <FileText size={12} />}</span><span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{p.title || tr("Sans titre")}</span></button>
        {canEdit && <span className="tacts">
          <button className="iconbtn" style={{ width: 26, height: 26 }} aria-label={tr("Ajouter une sous-page")} title={tr("Sous-page")} onClick={() => create(undefined, p.id)}><Plus size={14} /></button>
          <button className="iconbtn" style={{ width: 26, height: 26 }} aria-label={tr("Actions de la page")} title={tr("Actions")} onClick={() => setMenu(menu === p.id ? null : p.id)}><MoreHorizontal size={14} /></button>
        </span>}
        {menu === p.id && <div className="menu" style={{ right: 0, top: 30, minWidth: 190 }} onMouseLeave={() => setMenu(null)}>
          <button onClick={() => { setMenu(null); nav(`/pages/${p.id}`) }}><FileText size={16} />{tr("Ouvrir")}</button>
          <button onClick={() => renamePage(p)}><Type size={16} />{tr("Renommer")}</button>
          <button onClick={() => { setMenu(null); create(undefined, p.id) }}><Plus size={16} />{tr("Ajouter une sous-page")}</button>
          <button onClick={async () => { setMenu(null); try { const r = await api.post(`/api/pages/${p.id}/duplicate`); await reloadTree(); nav(`/pages/${r.id}`) } catch (e: any) { toast(e.message, true) } }}><Copy size={16} />{tr("Dupliquer")}</button>
          {canDelete && <button style={{ color: 'var(--tr)' }} onClick={() => removePage(p)}><Trash2 size={16} />{tr("Supprimer")}</button>}
        </div>}
      </div>
      {open && kids.map(k => <Node key={k.id} p={k} depth={depth + 1} />)}
    </>
  }

  return (
    <main className="content">
      <PageHead title={tr("Pages")} help={tr("Docs, règles et bases de données de la map, modifiés à plusieurs en temps réel.")}>
        {canEdit && <div style={{ position: 'relative' }}>
          <button className="btn" onClick={() => setTplOpen(o => !o)}>{tr("Modèles UEFN")}</button>
          {tplOpen && <div className="menu" style={{ right: 0, top: 48 }}>{templates?.map(t => <button key={t.key} onClick={() => create(t.key)}><span className="sico">{t.icon}</span>{t.title}</button>)}</div>}
        </div>}
        {canEdit && <button className="btn pri" onClick={() => create()}><Plus size={18} />{tr("Nouvelle page")}</button>}
      </PageHead>
      <div className="doc">
        <nav className="card tree" aria-label={tr("Arborescence")}>
          <div className="tsec">{tr("Favoris")}</div>
          {tree?.filter(p => p.favorite).map(p => <button key={p.id} className={'titem' + (p.id === id ? ' on' : '')} onClick={() => nav(`/pages/${p.id}`)}><Star size={14} />{p.title}</button>)}
          <div className="tsec">{tr("Pages de la map")}</div>
          {children(null).map(p => <Node key={p.id} p={p} depth={0} />)}
          {tree?.length === 0 && <Empty text={tr("Aucune page pour l'instant.")} />}
          <Trash />
        </nav>
        {id ? <Editor key={id} id={id} tree={tree ?? []} onNewChild={() => create(undefined, id)} /> : <div className="card"><Empty text={tr("Créez votre première page.")} /></div>}
      </div>
    </main>
  )
}

function Editor({ id, tree, onNewChild }: { id: number; tree: any[]; onNewChild: () => void }) {
  const app = useApp()
  const nav = useNav()
  const [page, setPage] = useState<any>(null)
  const [blocks, setBlocks] = useState<Block[]>([])
  const [title, setTitle] = useState('')
  const [focus, setFocus] = useState<{ id: string; n: number } | null>(null)
  const [slash, setSlash] = useState<{ id: string; q: string } | null>(null)
  const [mention, setMention] = useState<{ id: string } | null>(null)
  const [menuFor, setMenuFor] = useState<string | null>(null)
  const [dragId, setDragId] = useState<string | null>(null)
  const [overId, setOverId] = useState<string | null>(null)
  const [panel, setPanel] = useState<'comments' | 'versions' | 'share'>('comments')
  const [selBlock, setSelBlock] = useState<string | null>(null)
  const [saving, setSaving] = useState<'idle' | 'saving' | 'saved'>('idle')
  const [editors, setEditors] = useState<string[]>([])
  const clientId = useMemo(uid, [])
  const timer = useRef<any>(null)
  const dirty = useRef(false)
  const disabled = !app.canEdit

  const load = useCallback(async () => {
    const p = await api.get(`/api/pages/${id}`)
    setPage(p); setTitle(p.title); setBlocks(p.blocks.length ? p.blocks : [newBlock('p')])
  }, [id])
  useEffect(() => { load().catch(e => app.toast(e.message, true)) }, [load])
  useEffect(() => { if (page) app.send({ t: 'place', place: tr("Page · {0}", [page.title]) }) }, [page?.id])
  useEffect(() => app.subscribe(m => {
    if (m.t === 'page' && m.id === id && m.client !== clientId) {
      setEditors(e => Array.from(new Set([...e, m.name])))
      setTimeout(() => setEditors([]), 4000)
      if (!dirty.current) load()
    }
  }), [id, clientId, load])

  const save = (b: Block[], t: string) => {
    dirty.current = true
    setSaving('saving')
    clearTimeout(timer.current)
    timer.current = setTimeout(async () => {
      try { await api.patch(`/api/pages/${id}`, { blocks: b, title: t, client: clientId }); setSaving('saved') } catch (e: any) { app.toast(e.message, true) }
      dirty.current = false
    }, 700)
  }
  const update = (fn: (b: Block[]) => Block[]) => setBlocks(prev => { const n = fn(prev); save(n, title); return n })
  const setB = (bid: string, patch: Partial<Block>) => update(bs => bs.map(b => (b.id === bid ? { ...b, ...patch } : b)))
  const insertAfter = (bid: string, b: Block) => { update(bs => { const i = bs.findIndex(x => x.id === bid); const n = [...bs]; n.splice(i + 1, 0, b); return n }); setFocus({ id: b.id, n: Date.now() }) }
  const remove = (bid: string) => update(bs => { const i = bs.findIndex(x => x.id === bid); const n = bs.filter(x => x.id !== bid); if (n.length === 0) n.push(newBlock('p')); const prev = n[Math.max(0, i - 1)]; setFocus({ id: prev.id, n: Date.now() }); return n })

  const MD: [RegExp, string][] = [[/^# $/, 'h1'], [/^## $/, 'h2'], [/^### $/, 'h3'], [/^[-*] $/, 'bullet'], [/^1\. $/, 'number'], [/^\[\] $/, 'todo'], [/^> $/, 'quote'], [/^```$/, 'code'], [/^---$/, 'divider'], [/^! $/, 'callout']]
  const onText = (b: Block, v: string) => {
    for (const [re, type] of MD) {
      if (re.test(v) && b.type === 'p') { setB(b.id, { ...newBlock(type), id: b.id, text: '' }); setFocus({ id: b.id, n: Date.now() }); return }
    }
    if (v.startsWith('/')) setSlash({ id: b.id, q: v.slice(1).toLowerCase() }); else if (slash?.id === b.id) setSlash(null)
    if (v.endsWith('@')) setMention({ id: b.id }); else if (mention?.id === b.id && !v.includes('@')) setMention(null)
    setB(b.id, { text: v })
  }
  const onKey = (b: Block, e: React.KeyboardEvent<HTMLDivElement>) => {
    const el = e.currentTarget
    if (e.key === 'Enter' && !e.shiftKey && b.type !== 'code') {
      e.preventDefault()
      if (slash) { const first = INSERT.find(i => i[1].toLowerCase().includes(slash.q)); if (first) pick(b, first[0]); return }
      const listy = ['bullet', 'number', 'todo'].includes(b.type)
      if (listy && !el.innerText.trim()) { setB(b.id, { type: 'p' }); return }
      insertAfter(b.id, newBlock(listy ? b.type : 'p'))
    } else if (e.key === 'Backspace' && !el.innerText) {
      e.preventDefault()
      if (b.type !== 'p') setB(b.id, { type: 'p' }); else remove(b.id)
    } else if (e.key === 'Escape') { setSlash(null); setMention(null) }
  }
  const pick = (b: Block, type: string) => {
    setSlash(null)
    const nb = newBlock(type)
    setB(b.id, { ...nb, id: b.id })
    setFocus({ id: b.id, n: Date.now() })
  }
  const insertMention = (b: Block, label: string) => { setMention(null); setB(b.id, { text: (b.text ?? '').replace(/@$/, '') + label + ' ' }); setFocus({ id: b.id, n: Date.now() }) }

  const { data: comments, reload: reloadComments } = useLoad<any[]>(`/api/pages/${id}/comments`, [id])
  useLive(['comments'], reloadComments)
  const commented = new Set((comments ?? []).filter(c => !c.resolved).map(c => c.block))
  const { data: mem } = useLoad<any>(app.studio ? `/api/studios/${app.studio.id}/members` : null, [app.studio?.id])
  const { data: tasks } = useLoad<any[]>(app.map ? `/api/maps/${app.map.id}/tasks` : null, [app.map?.id])
  const { data: boards } = useLoad<any[]>(app.map ? `/api/maps/${app.map.id}/boards` : null, [app.map?.id])

  if (!page) return <div className="card editor"><p className="muted">{tr("Chargement…")}</p></div>
  let num = 0
  const headings = blocks.filter(b => ['h1', 'h2', 'h3'].includes(b.type))

  return <>
    <article className="card editor" aria-label={tr("Page {0}", [title])}>
      <div className="row" style={{ justifyContent: 'space-between', flexWrap: 'wrap', marginBottom: 18 }}>
        <div className="small muted">{tree.find(t => t.id === page.parent_id)?.title ? `${tree.find(t => t.id === page.parent_id)?.title} / ` : ''}{title || tr("Sans titre")}</div>
        <div className="row" style={{ gap: 8 }}>
          {editors.length > 0 && <span className="small muted">{editors.join(', ')} modifie…</span>}
          <span className="small muted">{saving === 'saving' ? tr("Enregistrement…") : saving === 'saved' ? tr("Enregistré") : tr("Modifié par {0} {1}", [page.updated_by ?? '—', ago(page.updated_at)])}</span>
          <button className="iconbtn" aria-label={page.favorite ? tr("Retirer des favoris") : tr("Ajouter aux favoris")} onClick={async () => { await api.patch(`/api/pages/${id}`, { favorite: !page.favorite }); setPage({ ...page, favorite: !page.favorite }) }}><Star size={18} fill={page.favorite ? 'currentColor' : 'none'} /></button>
          {!disabled && <button className="btn" onClick={onNewChild}>{tr("Sous-page")}</button>}
          {!disabled && <button className="iconbtn" aria-label={tr("Dupliquer la page")} title={tr("Dupliquer")} onClick={async () => { try { const r = await api.post(`/api/pages/${id}/duplicate`); nav(`/pages/${r.id}`) } catch (e: any) { app.toast(e.message, true) } }}><Copy size={18} /></button>}
          {['owner', 'admin', 'member'].includes(app.role) && <button className="iconbtn" aria-label={tr("Mettre la page à la corbeille")} title={tr("Supprimer (corbeille)")} onClick={async () => {
            const kids = tree.filter(x => x.parent_id === id).length
            if (!confirm(tr("Mettre « {0} »{1} à la corbeille ? Vous pourrez la restaurer.", [page.title || tr("Sans titre"), kids ? tr(" et ses {0} sous-page(s)", [kids]) : '']))) return
            try { await api.del(`/api/pages/${id}`); app.toast(tr("Page mise à la corbeille")); nav('/pages') } catch (e: any) { app.toast(e.message, true) }
          }}><Trash2 size={18} /></button>}
        </div>
      </div>
      <input className="field" style={{ width: 60, textAlign: 'center', fontSize: 20, height: 48, marginBottom: 8 }} defaultValue={page.icon} maxLength={2} aria-label={tr("Icône")} disabled={disabled}
        onBlur={e => api.patch(`/api/pages/${id}`, { icon: e.target.value })} />
      <input className="ptitle" value={title} placeholder={tr("Sans titre")} disabled={disabled} aria-label={tr("Titre de la page")} onChange={e => { setTitle(e.target.value); save(blocks, e.target.value) }} />
      <div className="stack" style={{ gap: 2, marginTop: 16 }}>
        {blocks.map(b => {
          if (b.type === 'number') num += 1; else num = 0
          const isText = TEXT_TYPES.includes(b.type)
          const fk = focus?.id === b.id ? String(focus.n) : undefined
          return (
            <div key={b.id} className={'blk' + (overId === b.id ? ' dragover' : '') + (commented.has(b.id) ? ' hasc' : '')}
              onDragOver={e => { if (dragId) { e.preventDefault(); setOverId(b.id) } }}
              onDrop={e => { e.preventDefault(); if (dragId && dragId !== b.id) update(bs => { const from = bs.find(x => x.id === dragId)!; const rest = bs.filter(x => x.id !== dragId); const i = rest.findIndex(x => x.id === b.id); rest.splice(i, 0, from); return rest }); setDragId(null); setOverId(null) }}
              onClick={() => setSelBlock(b.id)}>
              {!disabled && <button className="handle" draggable aria-label={tr("Déplacer ou modifier le bloc")} onDragStart={() => setDragId(b.id)} onDragEnd={() => { setDragId(null); setOverId(null) }} onClick={e => { e.stopPropagation(); setMenuFor(menuFor === b.id ? null : b.id) }}><GripVertical size={16} /></button>}
              {menuFor === b.id && (
                <div className="menu" style={{ left: 24, top: 28 }} onMouseLeave={() => setMenuFor(null)}>
                  <button onClick={() => { insertAfter(b.id, { ...b, id: uid() }); setMenuFor(null) }}><Copy size={16} />{tr("Dupliquer")}</button>
                  <button onClick={() => { remove(b.id); setMenuFor(null) }}><Trash2 size={16} />{tr("Supprimer")}</button>
                  <button onClick={() => { setSelBlock(b.id); setPanel('comments'); setMenuFor(null) }}><MessageSquare size={16} />{tr("Commenter")}</button>
                  <div className="small muted" style={{ padding: '6px 10px' }}>{tr("Transformer en")}</div>
                  {INSERT.filter(i => TEXT_TYPES.includes(i[0])).map(([t, l, , I]) => <button key={t} className={b.type === t ? 'on' : ''} onClick={() => { setB(b.id, { type: t }); setMenuFor(null) }}><I size={16} />{l}</button>)}
                </div>
              )}
              {b.type === 'bullet' && <span className="bullet" />}
              {b.type === 'number' && <span className="numbered">{num}.</span>}
              {b.type === 'todo' && <input type="checkbox" style={{ marginTop: 7 }} checked={!!b.checked} disabled={disabled} onChange={e => setB(b.id, { checked: e.target.checked })} aria-label={tr("Fait")} />}
              {b.type === 'toggle' && <button className="handle" style={{ opacity: 1 }} aria-label={b.open ? tr("Replier") : tr("Déplier")} onClick={() => setB(b.id, { open: !b.open })}>{b.open ? <ChevronDown size={16} /> : <ChevronRight size={16} />}</button>}
              {isText && (
                <div style={{ flex: 1, minWidth: 0 }}>
                  <Editable value={b.text ?? ''} focusKey={fk} disabled={disabled} onFocus={() => setSelBlock(b.id)}
                    className={({ h1: 'b-h1', h2: 'b-h2', h3: 'b-h3', quote: 'b-quote', callout: 'b-callout', code: 'b-code' } as any)[b.type] + (b.type === 'todo' && b.checked ? ' done' : '')}
                    ph={b.type === 'p' ? tr("Tapez « / » pour insérer un bloc, « @ » pour mentionner") : tr("Texte")}
                    onChange={v => onText(b, v)} onKeyDown={e => onKey(b, e)} />
                  {b.type === 'toggle' && b.open && <Editable value={b.body ?? ''} disabled={disabled} className="muted" ph={tr("Contenu masqué")} onChange={v => setB(b.id, { body: v })} />}
                  {slash?.id === b.id && (
                    <div className="menu slash" role="menu">
                      {INSERT.filter(i => (i[1] + i[2]).toLowerCase().includes(slash.q)).map(([t, l, h, I]) => (
                        <button key={t} role="menuitem" onMouseDown={e => { e.preventDefault(); pick(b, t) }}><span className="sico"><I size={16} /></span><span><span style={{ display: 'block' }}>{l}</span><span className="small muted">{h}</span></span></button>
                      ))}
                    </div>
                  )}
                  {mention?.id === b.id && (
                    <div className="menu slash">
                      <div className="small muted" style={{ padding: '4px 10px' }}>{tr("Personnes")}</div>
                      {mem?.members.map((m: any) => <button key={'u' + m.id} onMouseDown={e => { e.preventDefault(); insertMention(b, m.name.split(' ')[0]) }}>@{m.name}</button>)}
                      <div className="small muted" style={{ padding: '4px 10px' }}>{tr("Pages")}</div>
                      {tree.slice(0, 6).map(p => <button key={'p' + p.id} onMouseDown={e => { e.preventDefault(); insertMention(b, p.title) }}>@{p.title}</button>)}
                      <div className="small muted" style={{ padding: '4px 10px' }}>{tr("Tâches")}</div>
                      {tasks?.filter(t => t.status !== 'done').slice(0, 6).map(t => <button key={'t' + t.id} onMouseDown={e => { e.preventDefault(); insertMention(b, `#${t.number}`) }}>#{t.number} {t.title}</button>)}
                      <div className="small muted" style={{ padding: '4px 10px' }}>{tr("Date")}</div>
                      <button onMouseDown={e => { e.preventDefault(); insertMention(b, frDate(new Date().toISOString().slice(0, 10), { weekday: 'short', day: 'numeric', month: 'short' })) }}>@aujourd'hui</button>
                    </div>
                  )}
                </div>
              )}
              {b.type === 'divider' && <hr style={{ flex: 1, border: 0, borderTop: tr("1px solid var(--border)"), margin: '14px 0' }} />}
              {b.type === 'table' && <TableBlock b={b} disabled={disabled} onChange={rows => setB(b.id, { rows })} />}
              {['image', 'video', 'file'].includes(b.type) && <MediaBlock b={b} disabled={disabled} onChange={p => setB(b.id, p)} />}
              {b.type === 'columns' && <div style={{ flex: 1, display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 16 }}>{(b.cols ?? ['', '']).map((c, i) => <Editable key={i} value={c} disabled={disabled} ph={tr("Colonne {0}", [i + 1])} onChange={v => setB(b.id, { cols: (b.cols ?? ['', '']).map((x, j) => (j === i ? v : x)) })} />)}</div>}
              {b.type === 'toc' && <div style={{ flex: 1 }} className="b-callout"><div className="small muted">{tr("Sommaire")}</div>{headings.map(h => <div key={h.id} style={{ paddingLeft: h.type === 'h3' ? 28 : h.type === 'h2' ? 14 : 0 }}>{h.text}</div>)}</div>}
              {b.type === 'pagelink' && <div style={{ flex: 1 }}>{b.page ? <button className="btn" onClick={() => nav(`/pages/${b.page}`)}><FileText size={16} />{tree.find(t => t.id === b.page)?.title ?? tr("Page")}</button> : <select className="field" disabled={disabled} onChange={e => setB(b.id, { page: Number(e.target.value) })} defaultValue=""><option value="" disabled>{tr("Choisir une page…")}</option>{tree.filter(t => t.id !== id).map(t => <option key={t.id} value={t.id}>{t.title}</option>)}</select>}</div>}
              {b.type === 'board' && <div style={{ flex: 1 }}>{b.board ? <button className="btn" style={{ width: '100%', justifyContent: 'flex-start', height: 56 }} onClick={() => nav(`/tableau/${b.board}`)}><span className="ico" style={{ width: 34, height: 34 }}><PenTool size={16} /></span>{tr("Tableau blanc · {0}", [boards?.find(x => x.id === b.board)?.title ?? ''])}</button> : <select className="field" disabled={disabled} onChange={e => setB(b.id, { board: Number(e.target.value) })} defaultValue=""><option value="" disabled>{tr("Choisir un tableau…")}</option>{boards?.map(x => <option key={x.id} value={x.id}>{x.title}</option>)}</select>}</div>}
              {b.type === 'database' && <DatabaseBlock b={b} disabled={disabled} members={mem?.members ?? []} onChange={p => setB(b.id, p)} />}
            </div>
          )
        })}
        {!disabled && <button className="btn ghost" style={{ alignSelf: 'flex-start', marginTop: 8 }} onClick={() => insertAfter(blocks[blocks.length - 1].id, newBlock('p'))}><Plus size={16} />{tr("Ajouter un bloc")}</button>}
      </div>
    </article>
    <aside className="card side2" aria-label={tr("Panneau latéral")}>
      <Tabs value={panel} onChange={setPanel} items={[['comments', tr("Commentaires")], ['versions', tr("Versions")], ['share', tr("Partage")]]} />
      <div style={{ marginTop: 16 }}>
        {panel === 'comments' && <Comments pageId={id} blocks={blocks} sel={selBlock} comments={comments ?? []} reload={reloadComments} />}
        {panel === 'versions' && <Versions pageId={id} onRestored={load} />}
        {panel === 'share' && <Share page={page} setPage={setPage} />}
      </div>
    </aside>
  </>
}

function TableBlock({ b, disabled, onChange }: { b: Block; disabled: boolean; onChange: (r: string[][]) => void }) {
  const rows = b.rows ?? [['']]
  const set = (i: number, j: number, v: string) => onChange(rows.map((r, a) => (a === i ? r.map((c, k) => (k === j ? v : c)) : r)))
  return (
    <div style={{ flex: 1, overflowX: 'auto' }}>
      <table className="tbl dbtable"><tbody>{rows.map((r, i) => (
        <tr key={i}>{r.map((c, j) => <td key={j} style={i === 0 ? { fontWeight: 500 } : {}}><input value={c} disabled={disabled} onChange={e => set(i, j, e.target.value)} aria-label={tr("Cellule {0}, {1}", [i + 1, j + 1])} /></td>)}</tr>
      ))}</tbody></table>
      {!disabled && <div className="row" style={{ gap: 6, marginTop: 6 }}>
        <button className="btn ghost" onClick={() => onChange([...rows, rows[0].map(() => '')])}>{tr("+ ligne")}</button>
        <button className="btn ghost" onClick={() => onChange(rows.map(r => [...r, '']))}>{tr("+ colonne")}</button>
      </div>}
    </div>
  )
}

function MediaBlock({ b, disabled, onChange }: { b: Block; disabled: boolean; onChange: (p: Partial<Block>) => void }) {
  const safe = b.url && /^https:\/\//.test(b.url) ? b.url : null
  const yt = safe?.match(/(?:youtube\.com\/watch\?v=|youtu\.be\/)([\w-]{6,})/)
  return (
    <div style={{ flex: 1 }}>
      {!safe && <input className="field" style={{ width: '100%' }} placeholder={b.type === 'image' ? tr("Lien https de l'image") : b.type === 'video' ? tr("Lien YouTube ou vidéo (https)") : tr("Lien https du fichier")} disabled={disabled} onBlur={e => onChange({ url: e.target.value.trim() })} />}
      {safe && b.type === 'image' && <img src={safe} alt={b.text ?? ''} style={{ maxWidth: '100%', borderRadius: 12 }} />}
      {safe && b.type === 'video' && (yt ? <iframe title={tr("Vidéo")} src={`https://www.youtube-nocookie.com/embed/${yt[1]}`} style={{ width: '100%', aspectRatio: '16/9', border: 0, borderRadius: 12 }} allowFullScreen sandbox={tr("allow-scripts allow-same-origin allow-presentation")} /> : <video src={safe} controls style={{ width: '100%', borderRadius: 12 }} />)}
      {safe && b.type === 'file' && <a className="btn" href={safe} target="_blank" rel="noopener noreferrer"><Paperclip size={16} />{safe.split('/').pop()}</a>}
      {safe && !disabled && <button className="link small" style={{ marginLeft: 8 }} onClick={() => onChange({ url: '' })}>{tr("Changer")}</button>}
    </div>
  )
}

function DatabaseBlock({ b, disabled, members, onChange }: { b: Block; disabled: boolean; members: any[]; onChange: (p: Partial<Block>) => void }) {
  const cols = b.columns ?? []
  const rows: any[] = b.rows as any ?? []
  const [filter, setFilter] = useState('')
  const [sort, setSort] = useState<string | null>(null)
  const view = b.view ?? 'table'
  const statusCol = cols.find(c => c.type === 'select' || c.type === 'status')
  const dateCol = cols.find(c => c.type === 'date')
  let vis = rows.filter(r => !filter || JSON.stringify(r).toLowerCase().includes(filter.toLowerCase()))
  if (sort) vis = [...vis].sort((a, c) => String(a[sort] ?? '').localeCompare(String(c[sort] ?? ''), 'fr', { numeric: true }))
  const setRows = (r: any[]) => onChange({ rows: r } as any)
  const setCell = (rid: string, k: string, v: any) => setRows(rows.map(r => (r.id === rid ? { ...r, [k]: v } : r)))
  const addRow = (extra: any = {}) => setRows([...rows, { id: uid(), ...extra }])
  const groups = statusCol ? Array.from(new Set(rows.map(r => r[statusCol.key] || tr("Sans statut")))) : []
  const cell = (r: any, c: any) => {
    if (c.type === 'checkbox') return <input type="checkbox" checked={!!r[c.key]} disabled={disabled} onChange={e => setCell(r.id, c.key, e.target.checked)} />
    if (c.type === 'person') return <select className="field" style={{ height: 32 }} value={r[c.key] ?? ''} disabled={disabled} onChange={e => setCell(r.id, c.key, e.target.value)}><option value="" />{members.map(m => <option key={m.id} value={m.name.split(' ')[0]}>{m.name}</option>)}</select>
    return <input type={c.type === 'number' ? 'number' : c.type === 'date' ? 'date' : 'text'} value={r[c.key] ?? ''} disabled={disabled} onChange={e => setCell(r.id, c.key, c.type === 'number' ? Number(e.target.value) : e.target.value)} aria-label={c.label} />
  }
  return (
    <div style={{ flex: 1, border: tr("1px solid var(--border)"), borderRadius: 14, padding: 14, minWidth: 0 }}>
      <div className="row" style={{ justifyContent: 'space-between', flexWrap: 'wrap', gap: 8, marginBottom: 10 }}>
        <input className="field" style={{ fontWeight: 500, border: 0, padding: 0, height: 30 }} value={b.title ?? ''} disabled={disabled} onChange={e => onChange({ title: e.target.value })} aria-label={tr("Nom de la base")} />
        <div className="row" style={{ gap: 6, flexWrap: 'wrap' }}>
          <input className="field" style={{ height: 32, width: 140 }} placeholder={tr("Filtrer")} value={filter} onChange={e => setFilter(e.target.value)} />
          <select className="field" style={{ height: 32 }} value={sort ?? ''} onChange={e => setSort(e.target.value || null)} aria-label={tr("Trier")}><option value="">{tr("Tri")}</option>{cols.map(c => <option key={c.key} value={c.key}>{c.label}</option>)}</select>
        </div>
      </div>
      <div className="tabs" style={{ marginBottom: 10 }}>
        {[['table', tr("Tableau")], ['kanban', tr("Kanban")], ['calendar', tr("Calendrier")], ['timeline', tr("Chronologie")], ['gallery', tr("Galerie")], ['list', tr("Liste")]].map(([k, l]) => <button key={k} className={view === k ? 'on' : ''} onClick={() => onChange({ view: k })}>{l}</button>)}
      </div>
      {view === 'table' && <div style={{ overflowX: 'auto' }}><table className="tbl dbtable"><thead><tr>{cols.map(c => <th key={c.key}>{c.label}</th>)}</tr></thead><tbody>{vis.map(r => <tr key={r.id}>{cols.map(c => <td key={c.key}>{cell(r, c)}</td>)}</tr>)}</tbody></table></div>}
      {view === 'kanban' && statusCol && <div className="dbk">{groups.map(g => <div key={g} className="dbcol" onDragOver={e => e.preventDefault()} onDrop={e => setCell(e.dataTransfer.getData('text/plain'), statusCol.key, g)}><span className="badge">{g}</span>{vis.filter(r => (r[statusCol.key] || 'Sans statut') === g).map(r => <div key={r.id} className="dbcard" draggable onDragStart={e => e.dataTransfer.setData('text/plain', r.id)}><b style={{ fontWeight: 500 }}>{r[cols[0].key]}</b></div>)}</div>)}</div>}
      {view === 'calendar' && <div className="dbk">{dateCol ? [...vis].sort((a, c) => String(a[dateCol.key]).localeCompare(String(c[dateCol.key]))).map(r => <div key={r.id} className="dbcard row"><div style={{ width: 52, textAlign: 'center' }}><b style={{ fontSize: 18, fontWeight: 500 }}>{r[dateCol.key] ? new Date(r[dateCol.key] + 'T12:00').getDate() : '—'}</b><div className="small muted">{frDate(r[dateCol.key], { month: 'short' })}</div></div><span>{r[cols[0].key]}</span></div>) : <p className="muted">{tr("Ajoutez une propriété date.")}</p>}</div>}
      {view === 'timeline' && dateCol && (() => {
        const ds = vis.map(r => r[dateCol.key]).filter(Boolean).sort(); const min = new Date((ds[0] ?? '2026-01-01') + 'T12:00').getTime(); const max = new Date((ds[ds.length - 1] ?? '2026-01-02') + 'T12:00').getTime() + 864e5 * 7
        return <div className="stack" style={{ gap: 6 }}>{vis.map(r => { const x = r[dateCol.key] ? (new Date(r[dateCol.key] + 'T12:00').getTime() - min) / (max - min) : 0; return <div key={r.id} className="row"><span className="small" style={{ width: 160 }}>{r[cols[0].key]}</span><div style={{ flex: 1, position: 'relative', height: 22, background: 'var(--hover)', borderRadius: 6 }}><div style={{ position: 'absolute', left: `${x * 100}%`, width: '12%', top: 0, bottom: 0, background: 'var(--text)', borderRadius: 6 }} /></div></div> })}</div>
      })()}
      {view === 'gallery' && <div className="dbk">{vis.map(r => <div key={r.id} className="dbcard"><div style={{ height: 60, background: 'var(--hover)', borderRadius: 8, marginBottom: 8 }} /><b style={{ fontWeight: 500 }}>{r[cols[0].key]}</b>{cols.slice(1, 3).map(c => <div key={c.key} className="small muted">{String(r[c.key] ?? '')}</div>)}</div>)}</div>}
      {view === 'list' && <div className="list">{vis.map(r => <div key={r.id} className="li"><span style={{ flex: 1 }}>{r[cols[0].key]}</span>{statusCol && <span className="badge">{r[statusCol.key]}</span>}</div>)}</div>}
      {!disabled && <div className="row" style={{ gap: 6, marginTop: 8 }}>
        <button className="btn ghost" onClick={() => addRow()}>{tr("+ élément")}</button>
        <select className="field" style={{ height: 32 }} value="" onChange={e => { const t = e.target.value; if (t) onChange({ columns: [...cols, { key: uid(), label: tr("Propriété"), type: t }] }) }} aria-label={tr("Ajouter une propriété")}>
          <option value="">{tr("+ propriété")}</option>{['text', 'number', 'select', 'status', 'date', 'person', 'checkbox', 'url', 'file', 'relation', 'rollup', 'formula'].map(t => <option key={t} value={t}>{t}</option>)}
        </select>
      </div>}
    </div>
  )
}

function Comments({ pageId, blocks, sel, comments, reload }: { pageId: number; blocks: Block[]; sel: string | null; comments: any[]; reload: () => void }) {
  const { toast } = useApp()
  const [body, setBody] = useState('')
  const byBlock = new Map<string, any[]>()
  comments.forEach(c => byBlock.set(c.block, [...(byBlock.get(c.block) ?? []), c]))
  const selText = blocks.find(b => b.id === sel)?.text
  return <>
    {[...byBlock.entries()].map(([bid, list]) => {
      const res = list.every(c => c.resolved)
      return (
        <div key={bid} className="thread" style={{ opacity: res ? 0.55 : 1 }}>
          <div className="small muted" style={{ marginBottom: 6 }}>{tr("Sur « {0}… »", [(blocks.find(b => b.id === bid)?.text ?? tr("bloc supprimé")).slice(0, 50)])}</div>
          {list.map(c => <div key={c.id} className="row" style={{ alignItems: 'flex-start', padding: '6px 0' }}><Avatar name={c.who} small /><div><b style={{ fontWeight: 500 }}>{c.who?.split(' ')[0]}</b> <span className="small muted">{ago(c.at)}</span><div>{c.body}</div></div></div>)}
          <button className="btn ghost" onClick={async () => { await api.post(`/api/comments/${list[list.length - 1].id}/resolve`); reload() }}>{res ? tr("Rouvrir") : tr("Résoudre")}</button>
        </div>
      )
    })}
    {sel ? <form className="stack" onSubmit={async e => { e.preventDefault(); if (!body.trim()) return; try { await api.post(`/api/pages/${pageId}/comments`, { block: sel, body }); setBody(''); reload() } catch (er: any) { toast(er.message, true) } }}>
      <div className="small muted">{tr("Commenter « {0} »", [(selText ?? '').slice(0, 40)])}</div>
      <textarea className="field" style={{ minHeight: 70, padding: 10 }} value={body} onChange={e => setBody(e.target.value)} placeholder={tr("Votre commentaire, @ pour mentionner")} />
      <button className="btn pri">{tr("Commenter")}</button>
    </form> : <p className="small muted">{tr("Cliquez un bloc pour le commenter.")}</p>}
  </>
}

function Versions({ pageId, onRestored }: { pageId: number; onRestored: () => void }) {
  const { canEdit, toast } = useApp()
  const { data, reload } = useLoad<any[]>(`/api/pages/${pageId}/versions`, [pageId])
  return <>
    <div className="list">
      {data?.map(v => (
        <div key={v.id} className="li"><Avatar name={v.who} small /><div style={{ flex: 1, minWidth: 0 }}><div>{tr("Version {0}", [v.version])}</div><div className="small muted">{frDate(v.at, { day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' })} · <span className="mono">{v.sha256.slice(0, 10)}</span></div></div>
          {canEdit && <button className="btn" onClick={async () => { try { await api.post(`/api/pages/${pageId}/versions/${v.id}/restore`); toast(tr("Version restaurée")); onRestored(); reload() } catch (e: any) { toast(e.message, true) } }}><History size={16} />{tr("Restaurer")}</button>}</div>
      ))}
    </div>
    <p className="small muted">{tr("Chaque version est horodatée par le serveur avec une empreinte SHA-256 : c'est une preuve d'antériorité contre les copies de map.")}</p>
    <a className="btn" style={{ width: '100%' }} href={`/api/pages/${pageId}/proof`}><Download size={16} />{tr("Exporter l'historique daté")}</a>
  </>
}

function Share({ page, setPage }: { page: any; setPage: (p: any) => void }) {
  const { canEdit, toast } = useApp()
  const [link, setLink] = useState<string | null>(null)
  const set = async (share: string) => {
    try { const r = await api.patch(`/api/pages/${page.id}`, { share }); setPage({ ...page, share }); if (r.link) setLink(location.origin + r.link) } catch (e: any) { toast(e.message, true) }
  }
  return <div className="stack">
    {[['private', tr("Privé"), tr("Vous seul")], ['team', tr("Équipe"), tr("Tous les membres du studio")], ['link', tr("Lien en lecture"), tr("Toute personne qui a le lien")]].map(([k, l, d]) => (
      <label key={k} className="row" style={{ border: tr("1px solid var(--border)"), borderRadius: 12, padding: '10px 12px', alignItems: 'flex-start', cursor: 'pointer' }}>
        <input type="radio" name="share" checked={page.share === k} disabled={!canEdit} onChange={() => set(k)} /><span><span style={{ display: 'block' }}>{l}</span><span className="small muted">{d}</span></span>
      </label>
    ))}
    {link && <div className="stack" style={{ gap: 6 }}><span className="small muted">{tr("Lien de lecture (copiez-le maintenant, il ne sera plus affiché) :")}</span><input className="field mono" readOnly value={link} onFocus={e => e.target.select()} /></div>}
    {page.share === 'link' && !link && <p className="small muted">{tr("Un lien est actif. Repassez en « Équipe » pour le désactiver, puis en « Lien » pour en créer un nouveau.")}</p>}
    <div className="row"><a className="btn" style={{ flex: 1 }} href={`/api/pages/${page.id}/export.md`}><Download size={16} />{tr("Markdown")}</a><button className="btn" style={{ flex: 1 }} onClick={() => window.print()}><Share2 size={16} />PDF</button></div>
  </div>
}

function Trash() {
  const { map, role, toast } = useApp()
  const nav = useNav()
  const [open, setOpen] = useState(false)
  const { data, reload } = useLoad<any>(map && open ? `/api/maps/${map.id}/trash` : null, [map?.id, open])
  useLive(['pages', 'boards'], () => open && reload())
  const can = ['owner', 'admin', 'member'].includes(role)
  return <>
    <button className="titem" style={{ marginTop: 10 }} onClick={() => setOpen(o => !o)}><Trash2 size={14} />{tr("Corbeille{0}", [data ? ` (${data.pages.length})` : ''])}</button>
    {open && <div className="stack" style={{ gap: 2, paddingLeft: 8 }}>
      {data?.pages.length === 0 && <span className="small muted" style={{ padding: '4px 8px' }}>{tr("Vide")}</span>}
      {data?.pages.map((p: any) => <div key={p.id} className="row small" style={{ gap: 4 }}>
        <span style={{ flex: 1, minWidth: 0, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }} title={tr("Supprimée {0}", [ago(p.deleted_at)])}>{p.title || tr("Sans titre")}</span>
        {can && <button className="iconbtn" style={{ width: 28, height: 28 }} aria-label={tr("Restaurer")} title={tr("Restaurer")} onClick={async () => { await api.post(`/api/pages/${p.id}/restore`); reload(); nav(`/pages/${p.id}`) }}><RotateCcw size={14} /></button>}
        {can && <button className="iconbtn" style={{ width: 28, height: 28 }} aria-label={tr("Supprimer définitivement")} title={tr("Supprimer définitivement")} onClick={async () => { if (confirm(tr("Supprimer définitivement « {0} » ? Impossible à annuler.", [p.title]))) { try { await api.del(`/api/pages/${p.id}?purge=true`); reload() } catch (e: any) { toast(e.message, true) } } }}><X size={14} /></button>}
      </div>)}
    </div>}
  </>
}
