import type { ReactNode } from 'react'
import { initials } from '../api'

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

export const STATUS: Record<string, string> = { todo: 'À faire', doing: 'En cours', blocked: 'Bloqué', done: 'Fait' }
export const ROLE: Record<string, string> = { owner: 'Propriétaire', admin: 'Admin', member: 'Membre', freelance: 'Freelance', viewer: 'Invité en lecture' }
export const TAGS = ['code', 'carte', 'conception', 'test']

export const Empty = ({ text, action }: { text: string; action?: ReactNode }) => (
  <div className="empty"><p style={{ margin: '0 0 12px' }}>{text}</p>{action}</div>
)
