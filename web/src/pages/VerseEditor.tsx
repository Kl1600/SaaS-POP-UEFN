import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import * as Y from 'yjs'
import { MonacoBinding } from 'y-monaco'
import {
  FilePlus, Pencil, Trash2, Search, Send, History, Sparkles, X, ChevronRight, FileCode2, Lock, AlertTriangle, CheckCircle2, Circle, Loader2, MinusCircle, LifeBuoy,
} from 'lucide-react'
import { api, ago, frDate } from '../api'
import { useApp, useLoad } from '../state'
import { CardHead, Modal, PageHead } from '../components/ui'
import { setup, monaco, ctx as verseCtx, helpFor, MODULES, SNIPPETS } from '../verse'
import { diffLines, applyChoices, type Op } from '../diff'
import { useUefnStatus, STATUS_LABEL } from '../uefnState'

type FileInfo = { path: string; sha: string; updated_at: string; size: number; locked_by: string | null; pending: boolean }
type Diag = { path: string; line: number; col: number; end_line?: number; end_col?: number; severity: string; code: string; message: string }
type Open = { path: string; model: monaco.editor.ITextModel; doc: Y.Doc; base: string; saved: string; binding?: MonacoBinding }

const b64 = (u: Uint8Array) => { let s = ''; u.forEach(c => (s += String.fromCharCode(c))); return btoa(s) }
const unb64 = (s: string) => Uint8Array.from(atob(s), c => c.charCodeAt(0))
const uid = () => Math.random().toString(36).slice(2, 12)
const STEPS = ['Écriture des fichiers', 'Compilation dans UEFN', 'Erreurs et avertissements', 'Session de test', 'Archivage Lore']

export default function VerseEditor() {
  const app = useApp()
  const { map, me, canEdit, toast, subscribe, send: wsSend } = app
  const status = useUefnStatus()
  const { data: files, reload: reloadFiles } = useLoad<FileInfo[]>(map ? `/api/maps/${map.id}/verse/files` : null, [map?.id])
  const { data: tasks } = useLoad<any[]>(map ? `/api/maps/${map.id}/tasks` : null, [map?.id])
  const host = useRef<HTMLDivElement>(null)
  const editor = useRef<monaco.editor.IStandaloneCodeEditor | null>(null)
  const opened = useRef(new Map<string, Open>())
  const cid = useMemo(uid, [])
  const [tabs, setTabs] = useState<string[]>([])
  const [active, setActive] = useState<string | null>(null)
  const [dirty, setDirty] = useState<Record<string, boolean>>({})
  const [diags, setDiags] = useState<Diag[]>([])
  const [panel, setPanel] = useState<'ai' | 'history' | 'help' | null>(null)
  const [left, setLeft] = useState<'files' | 'search'>('files')
  const [task, setTask] = useState<number | ''>('')
  const [opts, setOpts] = useState({ push: true, lore: false, message: '' })
  const [run, setRun] = useState<{ id: string; steps: { status: string; detail?: any }[] } | null>(null)
  const [changedOnDisk, setChangedOnDisk] = useState<string | null>(null)
  const [newFile, setNewFile] = useState(false)
  const [remote, setRemote] = useState<Record<number, { name: string; path: string; line: number; col: number; t: number }>>({})
  const decos = useRef<monaco.editor.IEditorDecorationsCollection | null>(null)

  const info = (p: string | null) => files?.find(f => f.path === p)
  const lockedByOther = (p: string | null) => { const l = info(p)?.locked_by; return l && l.toLowerCase() !== me?.user.email.toLowerCase() ? l : null }

  // Monaco + symboles des digests
  useEffect(() => {
    setup()
    const e = monaco.editor.create(host.current!, {
      language: 'verse', theme: app.dark ? 'pop-dark' : 'pop-light', automaticLayout: true, minimap: { enabled: true }, fontSize: 14, fontFamily: "'Geist Mono', ui-monospace, monospace",
      folding: true, foldingStrategy: 'indentation', lineNumbers: 'on', renderWhitespace: 'boundary', tabSize: 4, insertSpaces: true, scrollBeyondLastLine: false, model: null,
    })
    editor.current = e
    decos.current = e.createDecorationsCollection([])
    let last = 0
    e.onDidChangeCursorPosition(ev => {
      if (Date.now() - last < 120) return
      last = Date.now()
      const p = e.getModel()?.uri.path.slice(1)
      if (p) wsSend({ t: 'cursor', place: 'verse:' + p, x: ev.position.lineNumber, y: ev.position.column })
    })
    return () => { opened.current.forEach(o => { o.binding?.destroy(); o.model.dispose(); o.doc.destroy() }); opened.current.clear(); e.dispose() }
  }, [])
  useEffect(() => { monaco.editor.setTheme(app.dark ? 'pop-dark' : 'pop-light') }, [app.dark])
  // Lien direct depuis la recherche globale : /verse?f=chemin&l=ligne
  const deep = useRef(false)
  useEffect(() => {
    const sp = new URLSearchParams(location.search)
    const f = sp.get('f')
    if (!deep.current && f && files?.some(x => x.path === f)) { deep.current = true; openFile(f, Number(sp.get('l')) || undefined) }
  }, [files])
  useEffect(() => {
    if (!map) return
    const load = () => api.get(`/api/maps/${map.id}/verse/symbols`).then(d => { verseCtx.symbols = d.symbols; verseCtx.tags = d.tags; verseCtx.devices = d.devices }).catch(() => {})
    load()
    return subscribe(m => { if (m.t === 'verse' && m.what === 'symbols') load() })
  }, [map?.id])

  const markDirty = (p: string) => { const o = opened.current.get(p); if (o) setDirty(d => ({ ...d, [p]: o.model.getValue() !== o.saved })) }

  // Ouvre un fichier : document Yjs partagé initialisé de façon identique chez tout le monde à partir du contenu de base.
  const openFile = useCallback(async (path: string, line?: number) => {
    let o = opened.current.get(path)
    if (!o) {
      const f = await api.get(`/api/maps/${map!.id}/verse/file?path=${encodeURIComponent(path)}`)
      const doc = new Y.Doc()
      const seed = new Y.Doc(); seed.clientID = 1
      seed.getText('t').insert(0, f.content)
      Y.applyUpdate(doc, Y.encodeStateAsUpdate(seed), 'remote')
      seed.destroy()
      const model = monaco.editor.createModel(f.content, 'verse', monaco.Uri.parse('file:///' + path))
      o = { path, model, doc, base: f.sha, saved: f.content }
      doc.on('update', (u: Uint8Array, origin: any) => { if (origin !== 'remote') wsSend({ t: 'yjs', path, update: b64(u), cid, base: o!.base }) })
      model.onDidChangeContent(() => markDirty(path))
      opened.current.set(path, o)
      wsSend({ t: 'yreq', path, cid })
    }
    setTabs(t => (t.includes(path) ? t : [...t, path]))
    setActive(path)
    const e = editor.current!
    if (e.getModel() !== o.model) {
      opened.current.forEach(x => { x.binding?.destroy(); x.binding = undefined })
      e.setModel(o.model)
      o.binding = new MonacoBinding(o.doc.getText('t'), o.model, new Set([e]))
    }
    e.updateOptions({ readOnly: !canEdit || !!lockedByOther(path) })
    if (line) { e.revealLineInCenter(line); e.setPosition({ lineNumber: line, column: 1 }); e.focus() }
  }, [map?.id, canEdit, files])

  const closeTab = (p: string) => {
    const o = opened.current.get(p)
    if (o && dirty[p] && !confirm(`${p} a des modifications non envoyées. Fermer quand même ?`)) return
    o?.binding?.destroy(); o?.model.dispose(); o?.doc.destroy(); opened.current.delete(p)
    const rest = tabs.filter(t => t !== p)
    setTabs(rest)
    if (active === p) { setActive(rest[rest.length - 1] ?? null); if (rest.length) openFile(rest[rest.length - 1]); else editor.current?.setModel(null) }
  }

  // Messages temps réel : Yjs, curseurs, fichiers changés sur le disque, étapes d'envoi.
  useEffect(() => subscribe(m => {
    if (m.t === 'yjs' && m.cid !== cid) {
      const o = opened.current.get(m.path)
      if (o && (m.base === o.base || m.full)) Y.applyUpdate(o.doc, unb64(m.update), 'remote')
    }
    if (m.t === 'yreq' && m.cid !== cid) {
      const o = opened.current.get(m.path)
      if (o) wsSend({ t: 'yjs', path: m.path, update: b64(Y.encodeStateAsUpdate(o.doc)), cid, base: o.base, full: true })
    }
    if (m.t === 'cursor' && typeof m.place === 'string' && m.place.startsWith('verse:') && m.uid !== me?.user.id) {
      setRemote(r => ({ ...r, [m.uid]: { name: m.name, path: m.place.slice(6), line: m.x, col: m.y, t: Date.now() } }))
    }
    if (m.t === 'verse' && (m.what === 'list' || m.what === 'file')) reloadFiles()
    if (m.t === 'verse' && m.what === 'file' && m.source === 'disk') {
      const o = opened.current.get(m.path)
      if (o && o.base !== m.sha) {
        if (o.model.getValue() === o.saved) reopen(m.path)
        else setChangedOnDisk(m.path)
      }
    }
    if (m.t === 'verse' && m.what === 'send') setRun(r => {
      if (!r || r.id !== m.run) return r
      const steps = [...r.steps]; steps[m.step - 1] = { status: m.status, detail: m.detail }
      return { ...r, steps }
    })
  }), [cid, subscribe, files])

  const reopen = async (path: string) => {
    const o = opened.current.get(path)
    if (!o) return
    o.binding?.destroy(); o.model.dispose(); o.doc.destroy(); opened.current.delete(path)
    setChangedOnDisk(null)
    await openFile(path)
    setDirty(d => ({ ...d, [path]: false }))
  }

  // Curseurs nommés des autres membres
  useEffect(() => {
    const e = editor.current
    if (!e || !active) return
    const list = Object.values(remote).filter(r => r.path === active && Date.now() - r.t < 20000)
    decos.current?.set(list.map(r => ({ range: new monaco.Range(r.line, r.col, r.line, r.col), options: { className: 'rcursor', after: { content: ' ' + r.name.split(' ')[0], inlineClassName: 'rcursor-name' }, stickiness: 1 } })))
  }, [remote, active])

  // Marqueurs d'erreurs à la bonne ligne, sur l'étendue exacte donnée par UEFN (#7) ; sans fin connue, le mot.
  useEffect(() => {
    opened.current.forEach(o => {
      monaco.editor.setModelMarkers(o.model, 'uefn', diags.filter(d => d.path === o.path).map(d => {
        const endLine = d.end_line ?? d.line
        const endCol = d.end_col ?? 0
        const exact = endCol > 0 && (endLine > d.line || endCol > d.col)
        const word = o.model.getWordAtPosition({ lineNumber: d.line, column: d.col })
        return {
        startLineNumber: d.line, startColumn: d.col, endLineNumber: exact ? endLine : d.line, endColumn: exact ? endCol : (word?.endColumn ?? d.col + 1), message: `${d.code ? d.code + ' · ' : ''}${d.message}`,
        severity: d.severity === 'error' ? monaco.MarkerSeverity.Error : monaco.MarkerSeverity.Warning,
      }}))
    })
  }, [diags, tabs])

  // Ctrl+S : envoyer à UEFN
  useEffect(() => {
    const h = (e: KeyboardEvent) => { if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') { e.preventDefault(); sendToUefn() } }
    window.addEventListener('keydown', h)
    return () => window.removeEventListener('keydown', h)
  })

  const sendToUefn = async () => {
    const list = [...opened.current.values()].filter(o => o.model.getValue() !== o.saved || o.path === active).filter(o => !lockedByOther(o.path))
    if (!list.length) return toast('Aucun fichier à envoyer.')
    const id = uid()
    setRun({ id, steps: STEPS.map(() => ({ status: 'waiting' })) })
    try {
      const r = await api.post(`/api/maps/${map!.id}/verse/send`, { files: list.map(o => ({ path: o.path, content: o.model.getValue() })), task: task || null, push: opts.push, lore: opts.lore, message: opts.message, run: id })
      if (r.queued) toast('UEFN n’est pas disponible : envoi mis en attente.')
      list.forEach(o => { o.saved = o.model.getValue(); markDirty(o.path) })
      if (r.diagnostics) { setDiags(r.diagnostics); if (r.errors) { setPanel('help'); toast(`${r.errors} erreur(s) de compilation`, true) } else if (r.push_error) toast(`Compilé, mais pas envoyé en session : ${r.push_error}`, true); else toast(r.pushed ? 'Compilé et envoyé en session' : 'Compilé sans erreur') }
      if (r.lore_error) toast(`Archivage Lore impossible : ${r.lore_error}`, true)
      reloadFiles()
    } catch (e: any) { toast(e.message, true) }
  }

  const fileAction = async (action: string, args: any, ok: string) => {
    try { const r = await api.post(`/api/maps/${map!.id}/uefn/action`, { action, args, task: task || null }); toast(r.queued ? 'Relais hors ligne : mis en attente' : ok); reloadFiles() } catch (e: any) { toast(e.message, true) }
  }
  const rename = async (p: string) => {
    const to = prompt('Nouveau chemin', p)
    if (!to || to === p) return
    await fileAction('rename_file', { path: p, to: to.endsWith('.verse') ? to : to + '.verse' }, 'Fichier renommé')
    if (opened.current.has(p)) closeTab(p)
  }
  const remove = async (p: string) => {
    if (!confirm(`Supprimer ${p} du projet ? (annulable depuis le journal de l’Organiseur)`)) return
    if (opened.current.has(p)) closeTab(p)
    await fileAction('delete_file', { path: p }, 'Fichier supprimé')
  }

  const activeDiags = diags.filter(d => d.path === active)
  const tree = useMemo(() => {
    const g = new Map<string, FileInfo[]>()
    files?.forEach(f => { const d = f.path.includes('/') ? f.path.slice(0, f.path.lastIndexOf('/')) : ''; g.set(d, [...(g.get(d) ?? []), f]) })
    return [...g.entries()].sort()
  }, [files])
  const others = Object.values(remote).filter(r => Date.now() - r.t < 20000)

  return (
    <main className="content">
      <PageHead title="Éditeur Verse" help="Le code Verse du projet UEFN, édité à plusieurs, compilé et envoyé en session depuis le navigateur.">
        <span className={'badge ' + (status.status === 'connected' ? 'b-g' : status.status === 'closed' ? 'b-o' : 'b-r')}>{STATUS_LABEL[status.status]}{status.pending ? ` · ${status.pending} en attente` : ''}</span>
      </PageHead>
      {status.status !== 'connected' && <div className="warn"><AlertTriangle size={18} /><span>{status.status === 'offline' ? 'Le relais UEFN Link n’est pas lancé : vous lisez le dernier état connu. Les écritures partiront à sa reconnexion.' : 'UEFN est fermé : les fichiers s’écrivent sur le disque, la compilation attendra l’ouverture d’UEFN.'}</span></div>}
      {changedOnDisk && <div className="warn"><AlertTriangle size={18} /><span>{changedOnDisk} a changé dans UEFN ou sur le disque pendant que vous l’éditiez.</span><button className="btn" onClick={() => reopen(changedOnDisk)}>Recharger (perdre mes changements)</button><button className="btn ghost" onClick={() => setChangedOnDisk(null)}>Garder les miens</button></div>}

      <div className="vgrid">
        <aside className="card vside">
          <div className="tabs" style={{ marginBottom: 10 }}><button className={left === 'files' ? 'on' : ''} onClick={() => setLeft('files')}>Fichiers</button><button className={left === 'search' ? 'on' : ''} onClick={() => setLeft('search')}><Search size={14} />Rechercher</button></div>
          {left === 'files' ? <>
            {canEdit && <button className="btn" style={{ width: '100%', marginBottom: 8 }} onClick={() => setNewFile(true)}><FilePlus size={16} />Nouveau fichier</button>}
            {tree.map(([dir, list]) => (
              <div key={dir}>
                <div className="tsec">{dir || 'Racine'}</div>
                {list.map(f => (
                  <div key={f.path} className={'vfile' + (active === f.path ? ' on' : '')}>
                    <button className="titem" onClick={() => openFile(f.path)} title={f.path}>
                      <FileCode2 size={15} /><span style={{ overflow: 'hidden', textOverflow: 'ellipsis' }}>{f.path.split('/').pop()}</span>
                      {dirty[f.path] && <span className="pip" style={{ background: 'var(--text)' }} />}{f.pending && <span className="small muted">attente</span>}{f.locked_by && <Lock size={13} />}
                    </button>
                    {canEdit && <span className="vfile-act"><button className="iconbtn" style={{ width: 26, height: 26 }} aria-label="Renommer" onClick={() => rename(f.path)}><Pencil size={13} /></button><button className="iconbtn" style={{ width: 26, height: 26 }} aria-label="Supprimer" onClick={() => remove(f.path)}><Trash2 size={13} /></button></span>}
                  </div>
                ))}
              </div>
            ))}
            {files?.length === 0 && <p className="small muted">Aucun fichier : lancez le relais UEFN Link sur le PC du projet.</p>}
          </> : <SearchPanel files={files ?? []} openFile={openFile} opened={opened} onChanged={p => markDirty(p)} />}
        </aside>

        <section className="card veditor">
          <div className="vtabs">
            {tabs.map(t => <div key={t} className={'vtab' + (t === active ? ' on' : '')}><button onClick={() => openFile(t)}>{t.split('/').pop()}{dirty[t] ? ' •' : ''}</button><button aria-label="Fermer" onClick={() => closeTab(t)}><X size={13} /></button></div>)}
            <span className="spacer" />
            {others.length > 0 && <span className="small muted" style={{ padding: '0 8px' }}>{others.map(o => `${o.name.split(' ')[0]} (${o.path.split('/').pop()}:${o.line})`).join(' · ')}</span>}
          </div>
          {active && lockedByOther(active) && <div className="small" style={{ padding: '6px 12px', background: 'var(--to-bg)' }}><Lock size={13} /> Verrouillé dans Lore par {lockedByOther(active)} : lecture seule.</div>}
          <div ref={host} className="vmonaco" style={{ display: active ? 'block' : 'none' }} />
          {!active && <div className="empty" style={{ height: 480, display: 'grid', placeItems: 'center' }}>Ouvrez un fichier à gauche.</div>}
          <div className="vbar">
            <select className="field" value={task} onChange={e => setTask(e.target.value ? Number(e.target.value) : '')} aria-label="Tâche liée" style={{ maxWidth: 260 }}>
              <option value="">Sans tâche</option>{tasks?.filter(t => t.status !== 'done').map(t => <option key={t.id} value={t.number}>#{t.number} {t.title}</option>)}
            </select>
            <label className="row small" style={{ gap: 6 }}><input type="checkbox" checked={opts.push} onChange={e => setOpts({ ...opts, push: e.target.checked })} />Session de test</label>
            <label className="row small" style={{ gap: 6 }}><input type="checkbox" checked={opts.lore} onChange={e => setOpts({ ...opts, lore: e.target.checked })} />Archiver dans Lore</label>
            {opts.lore && <input className="field" placeholder="Ce qui a été fait" value={opts.message} onChange={e => setOpts({ ...opts, message: e.target.value })} />}
            <span className="spacer" />
            <button className="btn" onClick={() => setPanel(panel === 'history' ? null : 'history')} disabled={!active}><History size={16} />Historique</button>
            <button className="btn" onClick={() => setPanel(panel === 'ai' ? null : 'ai')} disabled={!active}><Sparkles size={16} />Assistant</button>
            <button className="btn pri" onClick={sendToUefn} disabled={!canEdit || !active}><Send size={16} />Envoyer à UEFN</button>
          </div>
          {run && <div className="vsteps">{STEPS.map((s, i) => { const st = run.steps[i]?.status ?? 'waiting'; const I = st === 'done' ? CheckCircle2 : st === 'running' ? Loader2 : st === 'failed' ? AlertTriangle : st === 'skipped' ? MinusCircle : Circle; const why = st === 'failed' && typeof run.steps[i]?.detail === 'string' ? run.steps[i].detail : ''; return <span key={s} className={'vstep ' + st} title={why}><I size={15} className={st === 'running' ? 'spin' : ''} />{i + 1}. {s}{st === 'queued' ? ' (en attente)' : ''}{why && <span className="small"> : {why}</span>}</span> })}<button className="iconbtn" style={{ width: 26, height: 26, marginLeft: 'auto' }} aria-label="Fermer" onClick={() => setRun(null)}><X size={14} /></button></div>}
          {diags.length > 0 && <div className="vproblems">
            {diags.map((d, i) => (
              <button key={i} className="vprob" onClick={() => { openFile(d.path, d.line); setPanel('help') }}>
                <span className={'badge ' + (d.severity === 'error' ? 'b-r' : 'b-o')}>{d.severity === 'error' ? 'Erreur' : 'Avertissement'}</span>
                <span className="mono small">{d.path.split('/').pop()}:{d.line}</span><span style={{ flex: 1, textAlign: 'left' }}>{d.message}</span>
                {helpFor(d.code, d.message) && <span className="small muted"><LifeBuoy size={13} /> fiche</span>}
              </button>
            ))}
          </div>}
        </section>

        {panel && active && <aside className="card vpanel">
          <div className="row" style={{ justifyContent: 'space-between', marginBottom: 8 }}>
            <div className="tabs"><button className={panel === 'ai' ? 'on' : ''} onClick={() => setPanel('ai')}>Assistant</button><button className={panel === 'history' ? 'on' : ''} onClick={() => setPanel('history')}>Historique</button><button className={panel === 'help' ? 'on' : ''} onClick={() => setPanel('help')}>Aide</button></div>
            <button className="iconbtn" aria-label="Fermer" onClick={() => setPanel(null)}><X size={16} /></button>
          </div>
          {panel === 'ai' && <AiPanel path={active} model={opened.current.get(active)?.model} editor={editor.current} diags={activeDiags} task={task} />}
          {panel === 'history' && <HistoryPanel path={active} current={opened.current.get(active)?.model.getValue() ?? ''} onReverted={() => reopen(active)} />}
          {panel === 'help' && <HelpPanel diags={activeDiags.length ? activeDiags : diags} />}
        </aside>}
      </div>
      {newFile && <NewFileModal onClose={() => setNewFile(false)} onCreate={async (path, content) => { setNewFile(false); await fileAction('write_file', { path, content }, 'Fichier créé'); setTimeout(() => openFile(path), 300) }} />}
    </main>
  )
}

function SearchPanel({ files, openFile, opened, onChanged }: { files: FileInfo[]; openFile: (p: string, l?: number) => Promise<void>; opened: React.MutableRefObject<Map<string, Open>>; onChanged: (p: string) => void }) {
  const { map, canEdit, toast } = useApp()
  const [q, setQ] = useState('')
  const [rep, setRep] = useState('')
  const [regex, setRegex] = useState(false)
  const [cs, setCs] = useState(false)
  const [res, setRes] = useState<{ path: string; line: number; text: string }[]>([])
  const search = async () => {
    if (!q) return setRes([])
    const all: { path: string; content: string }[] = await api.get(`/api/maps/${map!.id}/verse/all`)
    let re: RegExp
    try { re = regex ? new RegExp(q, cs ? 'g' : 'gi') : new RegExp(q.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), cs ? 'g' : 'gi') } catch { return toast('Expression invalide', true) }
    const out: typeof res = []
    all.forEach(f => {
      const content = opened.current.get(f.path)?.model.getValue() ?? f.content
      content.split('\n').forEach((l, i) => { re.lastIndex = 0; if (re.test(l)) out.push({ path: f.path, line: i + 1, text: l.trim() }) })
    })
    setRes(out.slice(0, 500))
  }
  const replaceAll = async () => {
    const paths = [...new Set(res.map(r => r.path))]
    if (!confirm(`Remplacer dans ${paths.length} fichier(s) ? Les fichiers seront ouverts, à envoyer ensuite à UEFN.`)) return
    const re = regex ? new RegExp(q, cs ? 'g' : 'gi') : new RegExp(q.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), cs ? 'g' : 'gi')
    for (const p of paths) {
      await openFile(p)
      const m = opened.current.get(p)!.model
      m.pushEditOperations([], [{ range: m.getFullModelRange(), text: m.getValue().replace(re, rep) }], () => null)
      onChanged(p)
    }
    toast('Remplacé : pensez à « Envoyer à UEFN »')
    search()
  }
  return <div className="stack" style={{ gap: 8 }}>
    <input className="field" placeholder="Rechercher dans le projet" value={q} onChange={e => setQ(e.target.value)} onKeyDown={e => e.key === 'Enter' && search()} />
    {canEdit && <input className="field" placeholder="Remplacer par" value={rep} onChange={e => setRep(e.target.value)} />}
    <div className="row small" style={{ gap: 10 }}><label className="row" style={{ gap: 4 }}><input type="checkbox" checked={regex} onChange={e => setRegex(e.target.checked)} />Regex</label><label className="row" style={{ gap: 4 }}><input type="checkbox" checked={cs} onChange={e => setCs(e.target.checked)} />Casse</label></div>
    <div className="row"><button className="btn" onClick={search}>Rechercher</button>{canEdit && res.length > 0 && <button className="btn" onClick={replaceAll}>Tout remplacer</button>}</div>
    <div className="small muted">{res.length} résultat(s)</div>
    <div style={{ maxHeight: 420, overflow: 'auto' }}>{res.map((r, i) => <button key={i} className="titem" style={{ height: 'auto', padding: '6px 8px', display: 'block' }} onClick={() => openFile(r.path, r.line)}><span className="mono small muted">{r.path.split('/').pop()}:{r.line}</span><div className="mono small" style={{ whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{r.text}</div></button>)}</div>
    <p className="small muted">Astuces : F12 aller à la définition, F2 renommer partout, Ctrl+S envoyer à UEFN.</p>
  </div>
}

function AiPanel({ path, model, editor, diags, task }: { path: string; model?: monaco.editor.ITextModel; editor: monaco.editor.IStandaloneCodeEditor | null; diags: Diag[]; task: number | '' }) {
  const { map, canEdit, toast } = useApp()
  const [busy, setBusy] = useState(false)
  const [res, setRes] = useState<{ explanation: string; code?: string; page_id?: number } | null>(null)
  const [ops, setOps] = useState<Op[]>([])
  const [acc, setAcc] = useState<Set<number>>(new Set())
  const [doc, setDoc] = useState(true)
  const ask = async (mode: string) => {
    if (!model) return
    const sel = editor?.getSelection()
    setBusy(true); setRes(null)
    try {
      const r = await api.post(`/api/maps/${map!.id}/ai/verse`, { mode, path, content: model.getValue(), selection: sel && !sel.isEmpty() ? model.getValueInRange(sel) : null, diagnostics: diags, task: task || null, document: doc })
      setRes(r)
      if (r.code) { const o = diffLines(model.getValue(), r.code); setOps(o); setAcc(new Set(o.map((x, k) => (x.type === 'eq' ? -1 : k)).filter(k => k >= 0))) }
    } catch (e: any) { toast(e.message, true) } finally { setBusy(false) }
  }
  const apply = () => {
    if (!model) return
    model.pushEditOperations([], [{ range: model.getFullModelRange(), text: applyChoices(ops, acc) }], () => null)
    setRes(null); toast('Changements appliqués : envoyez à UEFN pour compiler')
  }
  const toggle = (k: number) => setAcc(a => { const n = new Set(a); n.has(k) ? n.delete(k) : n.add(k); return n })
  const changes = ops.filter(o => o.type !== 'eq').length
  return <div className="stack" style={{ gap: 10 }}>
    <div className="grid2" style={{ gridTemplateColumns: '1fr 1fr', gap: 8 }}>
      <button className="btn" disabled={busy} onClick={() => ask('explain')}>Expliquer</button>
      <button className="btn" disabled={busy || !canEdit} onClick={() => ask('fix')}>{diags.length ? 'Corriger les erreurs' : 'Corriger le fichier'}</button>
      <button className="btn" disabled={busy || !task || !canEdit} onClick={() => ask('write')} title={task ? '' : 'Choisissez une tâche en bas de l’éditeur'}>Écrire depuis #{task || '…'}</button>
      <button className="btn" disabled={busy || !canEdit} onClick={() => ask('review')}>Relire et documenter</button>
    </div>
    <label className="row small" style={{ gap: 6 }}><input type="checkbox" checked={doc} onChange={e => setDoc(e.target.checked)} />Créer une page de documentation (relecture)</label>
    <p className="small muted" style={{ margin: 0 }}>Moteur d’analyse Verse de POP, exécuté sur votre serveur : rien n’est envoyé à un service externe. Il suit la page « Règles et conventions ». Sélectionnez du code pour cibler l’explication.</p>
    {busy && <p className="muted"><Loader2 size={16} className="spin" /> Analyse en cours…</p>}
    {res && <>
      <div className="b-callout md" style={{ fontSize: 14, maxHeight: 320, overflow: 'auto' }}><Md text={res.explanation} /></div>
      {res.page_id && <a className="btn" href={`/pages/${res.page_id}`}>Ouvrir la page de documentation</a>}
      {res.code && <>
        <div className="row" style={{ justifyContent: 'space-between' }}><b style={{ fontWeight: 500 }}>Proposition · {changes} ligne(s) modifiée(s)</b><span className="row" style={{ gap: 6 }}><button className="link small" onClick={() => setAcc(new Set(ops.map((o, k) => (o.type === 'eq' ? -1 : k)).filter(k => k >= 0)))}>Tout accepter</button><button className="link small" onClick={() => setAcc(new Set())}>Tout refuser</button></span></div>
        <div className="vdiff">{ops.map((o, k) => o.type === 'eq' ? <div key={k} className="dl eq"><span /> {o.text}</div> :
          <label key={k} className={'dl ' + o.type + (acc.has(k) ? '' : ' off')}><input type="checkbox" checked={acc.has(k)} onChange={() => toggle(k)} aria-label="Accepter la ligne" />{o.type === 'add' ? '+' : '−'} {o.text}</label>)}</div>
        <button className="btn pri" onClick={apply} disabled={!canEdit}>Appliquer les lignes acceptées</button>
      </>}
    </>}
  </div>
}

function HistoryPanel({ path, current, onReverted }: { path: string; current: string; onReverted: () => void }) {
  const { map, canEdit, toast } = useApp()
  const { data, reload } = useLoad<any[]>(`/api/maps/${map!.id}/verse/versions?path=${encodeURIComponent(path)}`, [path])
  const [diff, setDiff] = useState<{ id: number; content: string } | null>(null)
  useEffect(() => { reload() }, [current.length])
  return <>
    <div className="list">{data?.map(v => (
      <div key={v.id} className="li" style={{ alignItems: 'flex-start' }}>
        <div style={{ flex: 1, minWidth: 0 }}><div className="small">{frDate(v.at, { day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' })} · {v.who ?? 'disque / UEFN'}{v.task ? ` · #${v.task}` : ''}</div><div className="small muted">{v.note}</div></div>
        <button className="btn" style={{ height: 32 }} onClick={async () => setDiff(await api.get(`/api/verse-versions/${v.id}`))}>Diff</button>
      </div>
    ))}</div>
    {diff && <DiffModal original={diff.content} modified={current} onClose={() => setDiff(null)} action={canEdit ? <button className="btn pri" onClick={async () => { try { const r = await api.post(`/api/verse-versions/${diff.id}/revert`); toast(r.queued ? 'Version restaurée (en attente du relais)' : 'Version restaurée et renvoyée à UEFN'); setDiff(null); onReverted() } catch (e: any) { toast(e.message, true) } }}>Revenir à cette version et la renvoyer à UEFN</button> : null} />}
  </>
}

function DiffModal({ original, modified, onClose, action }: { original: string; modified: string; onClose: () => void; action: React.ReactNode }) {
  const box = useRef<HTMLDivElement>(null)
  const { dark } = useApp()
  useEffect(() => {
    const d = monaco.editor.createDiffEditor(box.current!, { automaticLayout: true, readOnly: true, renderSideBySide: true, theme: dark ? 'pop-dark' : 'pop-light' } as any)
    const a = monaco.editor.createModel(original, 'verse'), b = monaco.editor.createModel(modified, 'verse')
    d.setModel({ original: a, modified: b })
    return () => { d.dispose(); a.dispose(); b.dispose() }
  }, [])
  return <Modal onClose={onClose} label="Comparer les versions">
    <div className="row" style={{ justifyContent: 'space-between', marginBottom: 10 }}><b style={{ fontWeight: 500 }}>Version choisie (gauche) · version actuelle (droite)</b><button className="iconbtn" aria-label="Fermer" onClick={onClose}><X size={16} /></button></div>
    <div ref={box} style={{ height: '60vh', minWidth: 'min(1000px, 80vw)' }} />
    <div className="row" style={{ justifyContent: 'flex-end', marginTop: 10 }}>{action}</div>
  </Modal>
}

function HelpPanel({ diags }: { diags: Diag[] }) {
  const sheets = diags.map(d => ({ d, h: helpFor(d.code, d.message) })).filter(x => x.h)
  return <div className="stack">
    {sheets.length === 0 && <p className="muted">Pas de fiche pour ces erreurs. Essayez « Corriger l’erreur » dans l’assistant.</p>}
    {sheets.map(({ d, h }, i) => (
      <div key={i} className="thread">
        <div style={{ fontWeight: 500 }}>{h!.title}</div>
        <div className="small muted" style={{ margin: '4px 0 8px' }}>Ligne {d.line} · {d.message}</div>
        <p style={{ margin: '0 0 8px' }}>{h!.why}</p>
        <div className="small muted">Avant</div><pre className="b-code" style={{ margin: '4px 0 8px' }}>{h!.before}</pre>
        <div className="small muted">Après</div><pre className="b-code" style={{ margin: '4px 0 0' }}>{h!.after}</pre>
      </div>
    ))}
  </div>
}

function NewFileModal({ onClose, onCreate }: { onClose: () => void; onCreate: (path: string, content: string) => void }) {
  const [name, setName] = useState('Content/nouveau_module.verse')
  const [tpl, setTpl] = useState('empty')
  const content = tpl === 'empty' ? '' : tpl.startsWith('m:') ? MODULES.find(m => m[0] === tpl.slice(2))![2] : SNIPPETS.find(s => s[0] === tpl.slice(2))![2].replace(/\$\{\d+:([^}]*)\}/g, '$1').replace(/\$\{?\d+\}?/g, '')
  return <Modal onClose={onClose} label="Nouveau fichier Verse">
    <form className="stack" onSubmit={e => { e.preventDefault(); onCreate(name.endsWith('.verse') ? name : name + '.verse', content) }}>
      <h2 style={{ margin: 0, fontWeight: 400 }}>Nouveau fichier Verse</h2>
      <label className="label">Chemin dans le projet<input className="field mono" value={name} onChange={e => setName(e.target.value)} required /></label>
      <label className="label">Partir de<select className="field" value={tpl} onChange={e => setTpl(e.target.value)}>
        <option value="empty">Fichier vide</option>
        <optgroup label="Modèles de modules">{MODULES.map(m => <option key={m[0]} value={'m:' + m[0]}>{m[1]}</option>)}</optgroup>
        <optgroup label="Snippets">{SNIPPETS.map(s => <option key={s[0]} value={'s:' + s[0]}>{s[1]}</option>)}</optgroup>
      </select></label>
      {content && <pre className="b-code" style={{ maxHeight: 220, overflow: 'auto' }}>{content}</pre>}
      <div className="row" style={{ justifyContent: 'flex-end' }}><button type="button" className="btn" onClick={onClose}>Annuler</button><button className="btn pri">Créer</button></div>
    </form>
  </Modal>
}

/** Mise en forme légère : titres ###, listes, **gras**, `code`. */
function Md({ text }: { text: string }) {
  const inline = (t: string) => t.split(/(\*\*[^*]+\*\*|`[^`]+`)/g).map((p, i) => p.startsWith('**') ? <b key={i} style={{ fontWeight: 600 }}>{p.slice(2, -2)}</b> : p.startsWith('`') ? <code key={i} className="mono">{p.slice(1, -1)}</code> : p)
  return <>{text.split('\n').map((l, i) => {
    if (l.startsWith('### ') || l.startsWith('## ') || l.startsWith('# ')) return <div key={i} style={{ fontWeight: 600, margin: '10px 0 4px' }}>{inline(l.replace(/^#+ /, ''))}</div>
    const m = l.match(/^(\s*)- (.*)$/)
    if (m) return <div key={i} style={{ paddingLeft: 12 + m[1].length * 6, textIndent: -10 }}>• {inline(m[2])}</div>
    return l.trim() ? <div key={i}>{inline(l)}</div> : <div key={i} style={{ height: 6 }} />
  })}</>
}
