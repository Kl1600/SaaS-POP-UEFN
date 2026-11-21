// Échelle de l'interface : styles.css pose un `zoom` sur <html> (--ui-zoom, 90 % sur grand écran).
// La souris (clientX/clientY) et getBoundingClientRect donnent des pixels d'écran ; les styles (left, transform…)
// sont en pixels CSS. Ces aides convertissent l'un vers l'autre, quel que soit le navigateur.

/** Pixels d'écran par pixel CSS (0,9 avec le zoom, 1 sans, ou dans un navigateur qui ne l'applique pas aux mesures). */
export function uiScale(): number {
  const b = document.body
  if (!b || !b.offsetWidth) return 1
  return b.getBoundingClientRect().width / b.offsetWidth || 1
}

/** Rectangle d'un élément en pixels CSS, et l'échelle `k` pour convertir clientX/clientY (diviser par k). */
export function cssRect(el: Element) {
  const r = el.getBoundingClientRect()
  const k = uiScale()
  return { left: r.left / k, top: r.top / k, right: r.right / k, bottom: r.bottom / k, width: r.width / k, height: r.height / k, k }
}
