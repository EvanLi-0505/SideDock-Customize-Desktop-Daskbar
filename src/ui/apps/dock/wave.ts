// macOS-like magnification ("wave") for the dock.
//
// The zoom is a continuous function of the cursor position, not of "which icon is
// hovered", so moving along the dock rolls a smooth wave instead of jumping from icon
// to icon:
//
//   zoom(x)  = 1 + (M - 1) * h * f(|x - c|)        f = cosine falloff over `range` icons
//   shift(x) = ∫ from c to x of the extra length     (neighbours make room)
//
// c (cursor) and h (hover amount 0..1) chase their targets with exponential smoothing,
// which gives the wave its springy follow. In full-width mode both ends of the dock stay
// put (far icons compress a little); in fit-content mode the bar grows instead, like the
// macOS Dock.
//
// Performance: while the wave moves, icons are promoted to compositor layers
// (`.waving` -> will-change: transform) and only transforms change, so frames are
// composited without repainting. When the wave comes to rest the promotion is dropped
// and the browser re-rasters the magnified icons at their final size, keeping them crisp.
// z-index and CSS variables are only written when their value changes. The name label is
// rendered by the separate tooltip window (LabelSink): keeping it out of the dock window
// keeps that transparent window small, which is what costs GPU/CPU per frame.
// Resting geometry is measured only when the layout changes (observers), never inside an
// animation frame: reading layout right after writing transforms would force a style
// recalculation per read.

/** Where the magnified icon is, in CSS px of the dock window. */
export interface LabelAnchor {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Receives the name label of the icon under the cursor (shown in the tooltip window). */
export interface LabelSink {
  show(text: string, anchor: LabelAnchor): void;
  move(anchor: LabelAnchor): void;
  hide(): void;
}

export interface WaveConfig {
  enabled: boolean;
  horizontal: boolean;
  side: "left" | "right" | "top" | "bottom";
  /** zoom of the icon under the cursor */
  scale: number;
  /** icons on each side that are magnified too */
  range: number;
  /** rendered icon size and distance between icon centers */
  size: number;
  pitch: number;
  /** full-width docks keep their ends fixed; fit-content docks grow */
  fixedEnds: boolean;
  /** extra room the bar may grow into (fit-content mode), px */
  freeLength: number;
  labels: boolean;
}

const FOLLOW_MS = 55; // cursor smoothing: lower = snappier
const GROW_MS = 90; // wave rising
const SETTLE_MS = 150; // wave falling back
const LARGE_ZOOM_FACTOR = 0.35; // media module grows less

/** Resting layout of the items, relative to the items container. */
interface Geometry {
  items: HTMLElement[];
  centers: number[];
  lengths: number[];
  left: number[];
  top: number[];
  width: number[];
  height: number[];
  /** container position in the viewport */
  originX: number;
  originY: number;
}

export class DockWave {
  private config: WaveConfig | null = null;
  private c = 0;
  private h = 0;
  private targetC = 0;
  private targetH = 0;
  private frame = 0;
  private last = 0;
  private suspended = false;
  private labelIndex = -1;
  private labelHidden = false;
  private lastPointer = "";
  private growKey = "";
  /** the bar background grew past the items (fit-content mode), in CSS px */
  onGrow: ((start: number, end: number) => void) | null = null;
  private labelPos = "";
  private labelShown = false;
  private geometry: Geometry | null = null;
  private readonly resizeObserver: ResizeObserver;
  private readonly mutationObserver: MutationObserver;
  private readonly invalidate = () => {
    this.geometry = null;
  };

  constructor(
    private readonly container: HTMLElement,
    private readonly bar: HTMLElement,
    private readonly label: LabelSink,
  ) {
    this.resizeObserver = new ResizeObserver(this.invalidate);
    this.resizeObserver.observe(container);
    this.resizeObserver.observe(bar);
    // items added / removed / reordered
    this.mutationObserver = new MutationObserver(this.invalidate);
    this.mutationObserver.observe(container, { childList: true, subtree: true });
    window.addEventListener("resize", this.invalidate);
  }

  configure(config: WaveConfig) {
    this.config = config;
    this.geometry = null;
    if (!config.enabled) this.reset();
  }

  pointer(clientX: number, clientY: number) {
    if (!this.config?.enabled || this.suspended) return;
    // a real move brings the label back after hideLabel()
    const key = `${clientX},${clientY}`;
    if (key !== this.lastPointer) {
      this.lastPointer = key;
      this.labelHidden = false;
    }
    const geo = this.measure();
    this.targetC = this.config.horizontal ? clientX - geo.originX : clientY - geo.originY;
    if (this.h < 0.05) this.c = this.targetC; // rise where the cursor entered
    this.targetH = 1;
    this.kick();
  }

  leave() {
    this.targetH = 0;
    this.kick();
  }

  /** Hides the name label until the cursor moves again (e.g. a popup opened). */
  hideLabel() {
    this.labelHidden = true;
    this.dropLabel();
  }

  private dropLabel() {
    this.labelIndex = -1;
    this.labelPos = "";
    if (this.labelShown) {
      this.labelShown = false;
      this.label.hide();
    }
  }

  /** Drag & drop needs the real layout: drop the wave immediately. */
  suspend(value: boolean) {
    this.suspended = value;
    if (value) this.reset();
  }

  destroy() {
    cancelAnimationFrame(this.frame);
    this.resizeObserver.disconnect();
    this.mutationObserver.disconnect();
    window.removeEventListener("resize", this.invalidate);
    this.reset();
  }

  private kick() {
    if (this.frame) return;
    this.container.classList.add("waving");
    this.last = performance.now();
    this.frame = requestAnimationFrame(this.step);
  }

  private step = (now: number) => {
    this.frame = 0;
    const dt = Math.min(64, now - this.last);
    this.last = now;
    this.c += (this.targetC - this.c) * (1 - Math.exp(-dt / FOLLOW_MS));
    const tau = this.targetH > this.h ? GROW_MS : SETTLE_MS;
    this.h += (this.targetH - this.h) * (1 - Math.exp(-dt / tau));

    const settled = Math.abs(this.targetC - this.c) < 0.3 && Math.abs(this.targetH - this.h) < 0.004;
    if (settled && this.targetH === 0) {
      this.reset();
      return;
    }
    this.apply();
    if (settled) {
      // at rest: drop the layers so the magnified icons are re-rastered sharp
      this.container.classList.remove("waving");
      this.settleLabel();
    } else {
      this.frame = requestAnimationFrame(this.step);
    }
  };

  private reset() {
    this.h = 0;
    this.targetH = 0;
    this.growKey = "";
    this.dropLabel();
    this.container.classList.remove("waving");
    for (const el of this.items()) {
      el.style.transform = "";
      el.style.zIndex = "";
    }
    this.bar.style.removeProperty("--grow-start");
    this.bar.style.removeProperty("--grow-end");
    this.onGrow?.(0, 0);
  }

  private items(): HTMLElement[] {
    return Array.from(this.container.querySelectorAll<HTMLElement>(".weg-item-drag-container"));
  }

  private measure(): Geometry {
    if (this.geometry) return this.geometry;
    const horizontal = !!this.config?.horizontal;
    const items = this.items();
    const rect = this.container.getBoundingClientRect();
    // offset* ignore transforms, so this is the resting layout even mid-wave
    const left = items.map((el) => el.offsetLeft);
    const top = items.map((el) => el.offsetTop);
    const width = items.map((el) => el.offsetWidth);
    const height = items.map((el) => el.offsetHeight);
    this.geometry = {
      items,
      left,
      top,
      width,
      height,
      centers: items.map((_, i) => (horizontal ? left[i]! + width[i]! / 2 : top[i]! + height[i]! / 2)),
      lengths: horizontal ? width : height,
      originX: rect.left,
      originY: rect.top,
    };
    return this.geometry;
  }

  /**
   * Transforms of every item for a wave of height `h` centered under the cursor.
   * Pure: nothing is written to the DOM.
   */
  private layout(geo: Geometry, h: number) {
    const cfg = this.config!;
    const width = (cfg.range + 1) * cfg.pitch; // falloff radius
    const amplitude = (cfg.size / cfg.pitch) * (cfg.scale - 1) * h;
    const falloff = (d: number) => (d >= width ? 0 : (1 + Math.cos((Math.PI * d) / width)) / 2);
    // ∫0..d falloff, closed form
    const area = (d: number) =>
      d >= width ? width / 2 : d / 2 + (width / (2 * Math.PI)) * Math.sin((Math.PI * d) / width);

    const { centers, lengths } = geo;
    const x0 = centers[0]! - lengths[0]! / 2;
    const x1 = centers.at(-1)! + lengths.at(-1)! / 2;

    // everything depends on where the wave is centered (w, in resting coordinates)
    const solve = (w: number) => {
      const shift = (x: number) => Math.sign(x - w) * amplitude * area(Math.abs(x - w));
      const s0 = shift(x0);
      const s1 = shift(x1);
      // how much of the end movement is cancelled (1 = ends fixed, 0 = bar grows)
      let anchor = 1;
      if (!cfg.fixedEnds) {
        const growth = s1 - s0;
        anchor = growth > 0 ? Math.min(1, Math.max(0, 1 - cfg.freeLength / growth)) : 0;
      }
      const correction = (x: number) => (x1 === x0 ? 0 : s0 + ((s1 - s0) * (x - x0)) / (x1 - x0));
      const offset = (x: number) => shift(x) - anchor * correction(x);
      return { s0, s1, anchor, offset };
    };

    // Keeping the ends fixed slides the whole wave a little; pick the center whose
    // *displayed* peak lands exactly under the cursor (converges in a few steps).
    let w = this.c;
    for (let i = 0; i < 4; i++) w = this.c - solve(w).offset(w);
    const { s0, s1, anchor, offset } = solve(w);

    const offsets = centers.map((x) => offset(x));
    const zooms = geo.items.map((el, i) => {
      if (el.dataset.kind === "separator") return 1;
      const factor = el.dataset.kind === "large" ? LARGE_ZOOM_FACTOR : 1;
      return 1 + (cfg.scale - 1) * h * falloff(Math.abs(centers[i]! - w)) * factor;
    });
    return { s0, s1, anchor, offsets, zooms };
  }

  private apply() {
    const cfg = this.config;
    if (!cfg) return;
    const geo = this.measure();
    const items = geo.items;
    if (!items.length) return;

    const { s0, s1, anchor, offsets, zooms } = this.layout(geo, this.h);
    let nearest = -1;
    let nearestDistance = Infinity;
    items.forEach((el, i) => {
      const moved = offsets[i]!;
      const zoom = zooms[i]!;
      el.style.transform = cfg.horizontal
        ? `translate3d(${moved.toFixed(2)}px,0,0) scale(${zoom.toFixed(4)})`
        : `translate3d(0,${moved.toFixed(2)}px,0) scale(${zoom.toFixed(4)})`;
      // the label names the icon that is displayed under the cursor
      const shown = Math.abs(geo.centers[i]! + moved - this.c);
      if (el.dataset.kind !== "separator" && el.dataset.label && shown < nearestDistance) {
        nearestDistance = shown;
        nearest = i;
      }
    });

    // stacking order only changes when the hovered icon changes: the hovered icon on
    // top, the rest of the wave below it (changing z-index forces a repaint)
    items.forEach((el, i) => {
      const z = i === nearest ? "3" : zooms[i]! > 1.001 ? "2" : "";
      if (el.style.zIndex !== z) el.style.zIndex = z;
    });

    const grow = 1 - anchor;
    const start = Math.max(0, -s0 * grow);
    const end = Math.max(0, s1 * grow);
    const growStart = `${start.toFixed(1)}px`;
    const growEnd = `${end.toFixed(1)}px`;
    const growKey = growStart + growEnd;
    if (growKey !== this.growKey) {
      // custom properties invalidate the whole subtree: only write real changes
      this.growKey = growKey;
      this.bar.style.setProperty("--grow-start", growStart);
      this.bar.style.setProperty("--grow-end", growEnd);
      this.onGrow?.(start, end);
    }

    this.placeLabel(geo, nearest);
  }

  /** Where the icon `index` is drawn for a given layout, in CSS px of the window. */
  private anchorOf(geo: Geometry, index: number, offsets: number[], zooms: number[]): LabelAnchor {
    const cfg = this.config!;
    const along = geo.centers[index]! + offsets[index]!;
    const zoom = zooms[index]!;
    const w = geo.width[index]! * zoom;
    const h = geo.height[index]! * zoom;
    const left = geo.left[index]!;
    const top = geo.top[index]!;
    // the magnified icon grows away from the screen edge (see transform-origin in CSS)
    let x: number;
    let y: number;
    switch (cfg.side) {
      case "left":
        x = left;
        y = along - h / 2;
        break;
      case "right":
        x = left + geo.width[index]! - w;
        y = along - h / 2;
        break;
      case "top":
        x = along - w / 2;
        y = top;
        break;
      default:
        x = along - w / 2;
        y = top + geo.height[index]! - h;
    }
    return { x: geo.originX + x, y: geo.originY + y, width: w, height: h };
  }

  /**
   * The label lives in another window, and every update is an IPC round-trip, so it is
   * only (re)placed when the hovered icon changes - directly at the spot the icon has
   * once the wave is fully up - plus one correction when the wave comes to rest.
   */
  private placeLabel(geo: Geometry, index: number) {
    const cfg = this.config!;
    if (!cfg.labels || this.labelHidden || index < 0 || this.targetH === 0 || this.h < 0.2) {
      this.dropLabel();
      return;
    }
    if (index === this.labelIndex && this.labelShown) return;
    const full = this.layout(geo, 1);
    const anchor = this.anchorOf(geo, index, full.offsets, full.zooms);
    this.labelIndex = index;
    this.labelShown = true;
    this.labelPos = `${anchor.x.toFixed(0)},${anchor.y.toFixed(0)}`;
    this.label.show(geo.items[index]!.dataset.label ?? "", anchor);
  }

  /** Final nudge once the wave is at rest (the cursor may have moved within the icon). */
  private settleLabel() {
    if (!this.labelShown || this.labelIndex < 0 || !this.config) return;
    const geo = this.measure();
    if (this.labelIndex >= geo.items.length) return;
    const { offsets, zooms } = this.layout(geo, this.h);
    const anchor = this.anchorOf(geo, this.labelIndex, offsets, zooms);
    const pos = `${anchor.x.toFixed(0)},${anchor.y.toFixed(0)}`;
    if (pos !== this.labelPos) {
      this.labelPos = pos;
      this.label.move(anchor);
    }
  }
}
