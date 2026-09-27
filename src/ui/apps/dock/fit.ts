// What the dock does when its items do not fit along its axis.
//
//   ShrinkThenStack  shrink icons down to MIN_SIZE, then overlap them like cards
//   ShrinkOnly       keep shrinking (down to HARD_MIN)
//   Collapse         shrink down to MIN_SIZE, then move the last apps into a "more" popup
//
// Pure math, no DOM: the result is applied through CSS variables.

import type { OverflowMode } from "@shared/types.ts";

/** icons are never shrunk below this before stacking/collapsing kicks in */
export const MIN_SIZE = 28;
/** absolute floor for extreme cases */
const HARD_MIN = 12;
/** at most this fraction of an icon may be covered by the next one */
const MAX_COVER = 0.72;

export interface FitInput {
  /** room for the items along the dock axis, in px */
  available: number;
  /** configured icon size */
  size: number;
  gap: number;
  mode: OverflowMode;
  /** regular one-slot items */
  normal: number;
  /** three-slot items (media module) */
  large: number;
  /** zero-length items that still take gaps */
  separators: number;
  /** app items that the Collapse mode is allowed to hide */
  collapsible: number;
}

export interface FitResult {
  /** icon size to render */
  size: number;
  /** how much each item slides under the previous one (px) */
  overlap: number;
  /** how many trailing apps go into the "more" popup */
  collapsed: number;
}

export function fitItems(input: FitInput): FitResult {
  const { available, size, gap, mode, normal, large, separators, collapsible } = input;
  const count = normal + large + separators;
  if (count === 0 || available <= 0) return { size, overlap: 0, collapsed: 0 };

  const units = normal + 3 * large; // icon-sized slots
  const junctions = count - 1;
  const fixed = junctions * gap + 2 * gap * large; // gaps, including the ones inside large items
  const lengthAt = (s: number) => units * s + fixed;

  if (units === 0 || lengthAt(size) <= available) return { size, overlap: 0, collapsed: 0 };

  const shrunk = (available - fixed) / units;
  const floor = Math.min(size, MIN_SIZE);

  if (mode === "ShrinkOnly") {
    return { size: Math.max(HARD_MIN, Math.min(size, shrunk)), overlap: 0, collapsed: 0 };
  }

  let s = Math.max(floor, Math.min(size, shrunk));
  if (lengthAt(s) <= available) return { size: s, overlap: 0, collapsed: 0 };

  if (mode === "Collapse" && collapsible > 0) {
    // every hidden app frees one slot, the "more" button takes one back
    const slot = s + gap;
    const needed = Math.ceil((lengthAt(s) - available + slot) / slot);
    if (needed <= collapsible) return { size: s, overlap: 0, collapsed: Math.max(needed, 1) };
    // not enough apps to hide: collapse them all and stack the rest
    const rest = fitItems({ ...input, mode: "ShrinkThenStack", normal: normal - collapsible + 1, collapsible: 0 });
    return { ...rest, collapsed: collapsible };
  }

  // stack: slide items under their neighbour
  if (junctions === 0) return { size: s, overlap: 0, collapsed: 0 };
  let overlap = (lengthAt(s) - available) / junctions;
  const maxOverlap = gap + MAX_COVER * s;
  if (overlap > maxOverlap) {
    // even fully stacked it does not fit: shrink under the stacking floor
    const denom = units - MAX_COVER * junctions;
    s = denom > 0 ? Math.max(HARD_MIN, (available - fixed + junctions * gap) / denom) : HARD_MIN;
    overlap = gap + MAX_COVER * s;
  }
  return { size: s, overlap, collapsed: 0 };
}
