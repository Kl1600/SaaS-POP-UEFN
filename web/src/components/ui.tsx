import type { ReactNode } from 'react'
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

export const Drawer = ({ onClose, label, children }: { onClose: () => void; label: string; children: ReactNode }) => (
  <>
    <div className="scrim" onClick={onClose} />
    <aside className="drawer" role="dialog" aria-label={label}>{children}</aside>
  </>
)

export const Modal = ({ onClose, label, children }: { onClose: () => void; label: string; children: ReactNode }) => (
  <>
    <div className="scrim" onClick={onClose} />
    <div className="modal" role="dialog" aria-label={label}>{children}</div>
  </>
)

export const STATUS: Record<string, string> = { todo: tr("À faire"), doing: tr("En cours"), blocked: tr("Bloqué"), done: tr("Fait") }
export const ROLE: Record<string, string> = { owner: tr("Propriétaire"), admin: tr("Admin"), member: tr("Membre"), freelance: tr("Freelance"), viewer: tr("Invité en lecture") }
export const TAGS = ['code', 'carte', 'conception', 'test']
/** Libellé affiché d'une étiquette de tâche (la valeur enregistrée reste en français). */
export const tagLabel = (t: string) => tr(t)

export const Empty = ({ text, action }: { text: string; action?: ReactNode }) => (
  <div className="empty"><p style={{ margin: '0 0 12px' }}>{text}</p>{action}</div>
)
