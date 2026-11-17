// Client HTTP : cookies de session + en-tête anti-CSRF sur chaque requête.
export class ApiError extends Error {
  status: number
  constructor(status: number, message: string) {
    super(message)
    this.status = status
  }
}

async function req<T>(method: string, url: string, body?: unknown): Promise<T> {
  let res: Response
  try { res = await fetch(url, {
    method,
    credentials: 'same-origin',
    headers: { 'x-pop': '1', ...(body !== undefined ? { 'content-type': 'application/json' } : {}) },
    body: body !== undefined ? JSON.stringify(body) : undefined,
  }) } catch {
    // « Load failed » (Safari) / « Failed to fetch » (Chrome) : le serveur POP ne répond pas.
    throw new ApiError(0, 'Le serveur POP ne répond pas. Vérifiez qu’il tourne toujours dans le terminal (./start.sh), puis réessayez.')
  }
  const text = await res.text()
  let data: any = null
  try { data = text ? JSON.parse(text) : null } catch { data = text }
  if (!res.ok) throw new ApiError(res.status, (data && data.error) || `Erreur ${res.status}`)
  return data as T
}

export const api = {
  get: <T = any>(u: string) => req<T>('GET', u),
  post: <T = any>(u: string, b?: unknown) => req<T>('POST', u, b ?? {}),
  put: <T = any>(u: string, b?: unknown) => req<T>('PUT', u, b ?? {}),
  patch: <T = any>(u: string, b?: unknown) => req<T>('PATCH', u, b ?? {}),
  del: <T = any>(u: string, b?: unknown) => req<T>('DELETE', u, b),
}

export const euros = (cents: number, digits = 2) =>
  (cents / 100).toLocaleString('fr-FR', { minimumFractionDigits: digits, maximumFractionDigits: digits }) + ' €'

export const frDate = (d?: string | null, opts: Intl.DateTimeFormatOptions = { day: 'numeric', month: 'short' }) => {
  if (!d) return '—'
  const x = new Date(d.length <= 10 ? d + 'T12:00:00' : d.replace(' ', 'T') + 'Z')
  return x.toLocaleDateString('fr-FR', opts)
}

export const ago = (d: string) => {
  const t = new Date(d.replace(' ', 'T') + 'Z').getTime()
  const s = Math.round((Date.now() - t) / 1000)
  if (s < 60) return "à l'instant"
  if (s < 3600) return `il y a ${Math.round(s / 60)} min`
  if (s < 86400) return `il y a ${Math.round(s / 3600)} h`
  if (s < 172800) return 'hier'
  return frDate(d)
}

export const initials = (name?: string | null) =>
  (name || '?').split(/\s+/).filter(Boolean).slice(0, 2).map(w => w[0]!.toUpperCase()).join('')

export const ymd = (d: Date) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
export const addDays = (s: string, n: number) => { const d = new Date(s + 'T12:00:00'); d.setDate(d.getDate() + n); return ymd(d) }
