// Diff ligne à ligne (plus longue sous-séquence commune) pour accepter ou refuser chaque ligne.
export type Op = { type: 'eq' | 'del' | 'add'; text: string; a?: number; b?: number }

export function diffLines(a: string, b: string): Op[] {
  const A = a.split('\n'), B = b.split('\n')
  const n = A.length, m = B.length
  if (n * m > 4_000_000) return [...A.map(t => ({ type: 'del' as const, text: t })), ...B.map(t => ({ type: 'add' as const, text: t }))]
  const L: Uint32Array[] = Array.from({ length: n + 1 }, () => new Uint32Array(m + 1))
  for (let i = n - 1; i >= 0; i--) for (let j = m - 1; j >= 0; j--) L[i][j] = A[i] === B[j] ? L[i + 1][j + 1] + 1 : Math.max(L[i + 1][j], L[i][j + 1])
  const out: Op[] = []
  let i = 0, j = 0
  while (i < n && j < m) {
    if (A[i] === B[j]) { out.push({ type: 'eq', text: A[i], a: i + 1, b: j + 1 }); i++; j++ }
    else if (L[i + 1][j] >= L[i][j + 1]) { out.push({ type: 'del', text: A[i], a: i + 1 }); i++ }
    else { out.push({ type: 'add', text: B[j], b: j + 1 }); j++ }
  }
  while (i < n) { out.push({ type: 'del', text: A[i], a: i + 1 }); i++ }
  while (j < m) { out.push({ type: 'add', text: B[j], b: j + 1 }); j++ }
  return out
}

/** Reconstruit le texte : une ligne supprimée acceptée disparaît, une ligne ajoutée acceptée apparaît. */
export function applyChoices(ops: Op[], accepted: Set<number>) {
  const out: string[] = []
  ops.forEach((o, k) => {
    if (o.type === 'eq') out.push(o.text)
    else if (o.type === 'del' && !accepted.has(k)) out.push(o.text)
    else if (o.type === 'add' && accepted.has(k)) out.push(o.text)
  })
  return out.join('\n')
}
