import { createPortal } from 'react-dom'
import { useEffect, useRef, type ReactNode } from 'react'
import { initials } from '../api'
import { tr } from '../i18n/t'

export const Avatar = ({ name, small }: { name?: string | null; small?: boolean }) => (
  <span className={'avatar' + (small ? ' av-s' : '')} title={name ?? ''} aria-label={name ?? ''}>{initials(name)}</span>
)

export const PageHead = ({ title, help, children }: { title: string; help: string; children?: ReactNode }) => (
  <div className="phead">
    <div><h1>{title}</h1><p>{help}</p></div>
    {children && <div className="row" style={{ flexWrap: 'wrap' }}>{children}</div>}
  </div>
)

export const CardHead = ({ icon, title, sub, children }: { icon?: ReactNode; title: string; sub?: string; children?: ReactNode }) => (
  <div className="chead">
    <div><h2>{icon}{title}</h2>{sub && <p className="sub">{sub}</p>}</div>
    {children && <div className="row" style={{ flexWrap: 'wrap', gap: 8 }}>{children}</div>}
  </div>
)

export const Tabs = <T extends string>({ value, onChange, items }: { value: T; onChange: (v: T) => void; items: [T, string][] }) => (
  <div className="tabs" role="tablist">
    {items.map(([k, l]) => (
      <button key={k} role="tab" aria-selected={value === k} className={value === k ? 'on' : ''} onClick={() => onChange(k)}>{l}</button>
    ))}
  </div>
)

/** Fenêtre au clavier : le focus entre dans la fenêtre, Tab y reste, Échap ferme, le focus revient ensuite où il était. */
export function useDialog(onClose: () => void) {
  const ref = useRef<HTMLElement>(null)
  const close = useRef(onClose); close.current = onClose
  useEffect(() => {
    const prev = document.activeElement as HTMLElement | null
    const box = ref.current
    const items = () => Array.from(box?.querySelectorAll<HTMLElement>('a[href], button:not([disabled]), input:not([disabled]):not([type=hidden]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])') ?? []).filter(e => e.offsetParent !== null)
    if (box && !box.contains(document.activeElement)) (box.querySelector<HTMLElement>('[autofocus]') ?? items()[0] ?? box).focus()
    const key = (e: KeyboardEvent) => {
      if (e.key === 'Escape') { e.stopPropagation(); close.current() }
      if (e.key === 'Tab' && box) {
        const f = items(); if (!f.length) return
        const first = f[0], last = f[f.length - 1]
        if (e.shiftKey && document.activeElement === first) { e.preventDefault(); last.focus() }
        else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first.focus() }
      }
    }
    document.addEventListener('keydown', key)
    return () => { document.removeEventListener('keydown', key); if (prev && document.contains(prev)) prev.focus() }
  }, [])
  return ref
}

/** Rend une fiche au niveau de l'appli (dans `.app`, pour garder le thème) : centrée sur l'écran,
 *  quel que soit le conteneur de la page (un parent animé ou transformé décalerait `position: fixed`). */
export const Layer = ({ children }: { children: ReactNode }) => createPortal(children, document.querySelector('.app') ?? document.body)

export const Drawer = ({ onClose, label, children }: { onClose: () => void; label: string; children: ReactNode }) => {
  const ref = useDialog(onClose)
  return <Layer>
    <div className="scrim" onClick={onClose} />
    <aside ref={ref as any} className="drawer" role="dialog" aria-modal="true" aria-label={label} tabIndex={-1}>{children}</aside>
  </Layer>
}

export const Modal = ({ onClose, label, children }: { onClose: () => void; label: string; children: ReactNode }) => {
  const ref = useDialog(onClose)
  return <>
    <div className="scrim" onClick={onClose} />
    <div ref={ref as any} className="modal" role="dialog" aria-modal="true" aria-label={label} tabIndex={-1}>{children}</div>
  </>
}

export const STATUS: Record<string, string> = { todo: tr("À faire"), doing: tr("En cours"), blocked: tr("Bloqué"), done: tr("Fait") }
export const ROLE: Record<string, string> = { owner: tr("Propriétaire"), admin: tr("Admin"), member: tr("Membre"), freelance: tr("Freelance"), viewer: tr("Invité en lecture") }
export const TAGS = ['code', 'carte', 'conception', 'test']
/** Libellé affiché d'une étiquette de tâche (la valeur enregistrée reste en français). */
export const tagLabel = (t: string) => tr(t)

export const Empty = ({ text, action }: { text: string; action?: ReactNode }) => (
  <div className="empty"><p style={{ margin: '0 0 12px' }}>{text}</p>{action}</div>
)
