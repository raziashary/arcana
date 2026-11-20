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
    .sort((a, b) => b.priority - a.priority || a.id - b.id)
  for (const box of ordered) {
    if (kept.some(other => overlaps(box, other, pad))) hidden.add(box.id)
    else kept.push(box)
  }
  return hidden
}
