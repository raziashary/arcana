// Which City labels to hide so the rest can be read.
//
// A district label is sized in screen pixels from the district's own width, with
// a floor, so in a city of dozens of small districts the labels overlap however
// far the camera is (district and label both scale with distance, so zooming out
// does not help). The fix works on what the user sees: measure each label's
// on-screen box, keep labels in priority order, and hide any that would overlap
// one already kept. Nothing about the layout or the label sizes is assumed.

export interface LabelBox {
  id: number
  /** Higher keeps its place: a landmark or a large district outranks a small one. */
  priority: number
  /** Breaks a priority tie the same way on every pass (the label's own text), so
   *  which of two equal labels survives does not depend on DOM order. */
  tie?: string
  left: number
  top: number
  right: number
  bottom: number
}

function overlaps(a: LabelBox, b: LabelBox, pad: number): boolean {
  return (
    a.left < b.right + pad &&
    a.right + pad > b.left &&
    a.top < b.bottom + pad &&
    a.bottom + pad > b.top
  )
}

/** Ids to hide. Empty boxes (a label that has not been laid out yet) are ignored. */
export function hiddenLabels(boxes: LabelBox[], pad = 4): Set<number> {
  const hidden = new Set<number>()
  const kept: LabelBox[] = []
  const ordered = boxes
    .filter(box => box.right > box.left && box.bottom > box.top)
    .sort(
      (a, b) =>
        b.priority - a.priority ||
        (a.tie ?? '').localeCompare(b.tie ?? '') ||
        a.id - b.id,
    )
  for (const box of ordered) {
    if (kept.some(other => overlaps(box, other, pad))) hidden.add(box.id)
    else kept.push(box)
  }
  return hidden
}

/**
 * Measure every `[data-city-label]` under `root` and set `visibility` so that no
 * visible label overlaps a more important one. A label that is not laid out yet
 * (an empty box) is left visible. Returns how many labels it hid.
 */
export function declutterLabels(root: ParentNode, pad = 4): number {
  const labels = Array.from(root.querySelectorAll<HTMLElement>('[data-city-label]'))
  const boxes: LabelBox[] = labels.map((el, id) => {
    const rect = el.getBoundingClientRect()
    return {
      id,
      priority: Number(el.dataset.priority ?? 0),
      tie: el.dataset.cityLabel ?? '',
      left: rect.left,
      top: rect.top,
      right: rect.right,
      bottom: rect.bottom,
    }
  })
  const hidden = hiddenLabels(boxes, pad)
  labels.forEach((el, id) => {
    const next = hidden.has(id) ? 'hidden' : 'visible'
    if (el.style.visibility !== next) el.style.visibility = next
  })
  return hidden.size
}
