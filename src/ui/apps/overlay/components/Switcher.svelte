<!-- Stage-manager-like window switcher. The selected window is shown large in the
     center (with the other windows of the same app below it), every app is a card on
     the left. Previews are live DWM thumbnails: this page only computes their rects
     (CSS px) and draws the frames, labels and selection around them.

     Keyboard: the switcher shortcut (backend hotkey) steps forward; Tab / arrows move,
     Enter switches, Esc closes. Releasing the shortcut's modifier after several presses
     switches (handled by the backend, see widgets/overlay.rs). -->
<script lang="ts">
  import { onMount } from "svelte";
  import { t } from "@shared/i18n/index.svelte.ts";
  import { api, Events, iconUrl, on } from "@shared/ipc.ts";
  import type { ThumbnailRequest, UserAppWindow } from "@shared/types.ts";
  import { wheelStepper, type Direction } from "../wheel.ts";

  interface Box {
    x: number;
    y: number;
    width: number;
    height: number;
  }

  let windows = $state<UserAppWindow[]>([]);
  // most recently used first, frozen when the switcher opens (focusing the overlay
  // itself must not reorder the list)
  let order: number[] = [];
  let selected = $state(0);
  let viewport = $state({ width: window.innerWidth, height: window.innerHeight });

  function accept(list: UserAppWindow[]) {
    if (!order.length) {
      order = [...list]
        .sort((a, b) => b.lastForegroundAt - a.lastForegroundAt)
        .map((w) => w.hwnd);
      selected = list.length > 1 ? 1 : 0;
    } else {
      // closed windows leave, new ones go last
      for (const w of list) if (!order.includes(w.hwnd)) order.push(w.hwnd);
    }
    windows = list;
  }

  const ordered = $derived.by(() => {
    const byHwnd = new Map(windows.map((w) => [w.hwnd, w]));
    return order.map((h) => byHwnd.get(h)).filter((w): w is UserAppWindow => !!w);
  });

  $effect(() => {
    if (selected > ordered.length - 1) selected = Math.max(0, ordered.length - 1);
  });

  const current = $derived(ordered[selected]);

  $effect(() => {
    if (current) api.switcherSelect(current.hwnd);
  });

  onMount(() => {
    api.getWindows().then(accept);
    const changed = on<UserAppWindow[]>(Events.WindowsChanged, accept);
    const step = on<number>(Events.SwitcherStep, () => next(1));
    const resize = () => (viewport = { width: window.innerWidth, height: window.innerHeight });
    window.addEventListener("resize", resize);
    // not passive: the wheel must not scroll anything natively
    window.addEventListener("wheel", onWheel, { passive: false });
    return () => {
      window.removeEventListener("wheel", onWheel);
      stepper.dispose();
      changed.then((f) => f());
      step.then((f) => f());
      window.removeEventListener("resize", resize);
    };
  });

  function next(delta: number) {
    const n = ordered.length;
    if (n) selected = (((selected + delta) % n) + n) % n;
  }

  /** Next / previous window of the selected app (wheel over the stage). */
  function stepInApp(direction: Direction) {
    const group = currentGroup;
    if (!group || group.length < 2 || !current) return;
    const i = group.indexOf(current);
    const next = group[(i + direction + group.length) % group.length]!;
    selected = ordered.indexOf(next);
  }

  /** Next / previous app (wheel over the app cards). */
  function stepApp(direction: Direction) {
    const n = groups.length;
    if (!n) return;
    const i = currentGroup ? groups.indexOf(currentGroup) : 0;
    const next = groups[(i + direction + n) % n]!;
    selected = ordered.indexOf(next[0]!);
  }

  // over the app cards the wheel moves between apps, anywhere else between the windows
  // of the selected app
  let wheelTarget: "apps" | "windows" = "windows";
  const stepper = wheelStepper((direction) =>
    wheelTarget === "apps" ? stepApp(direction) : stepInApp(direction),
  );

  function onWheel(e: WheelEvent) {
    if (e.ctrlKey) return;
    e.preventDefault();
    wheelTarget = e.clientX < MARGIN + stripWidth + 28 ? "apps" : "windows";
    const horizontal = Math.abs(e.deltaX) > Math.abs(e.deltaY);
    stepper.handle(e, horizontal ? e.deltaX : e.deltaY);
  }

  function activate(w: UserAppWindow | undefined) {
    if (w) api.activateWindow(w.hwnd);
  }

  function onKeydown(e: KeyboardEvent) {
    switch (e.key) {
      case "Escape":
        e.preventDefault();
        api.overlayHide();
        break;
      case "Enter":
        e.preventDefault();
        activate(current);
        break;
      case "Tab":
        e.preventDefault();
        next(e.shiftKey ? -1 : 1);
        break;
      case "ArrowDown":
      case "ArrowRight":
        e.preventDefault();
        next(1);
        break;
      case "ArrowUp":
      case "ArrowLeft":
        e.preventDefault();
        next(-1);
        break;
    }
  }

  // ---------------- grouping ----------------

  const appKey = (w: UserAppWindow) => (w.path ?? w.umid ?? w.appName).toLowerCase();

  const groups = $derived.by(() => {
    const map = new Map<string, UserAppWindow[]>();
    for (const w of ordered) {
      const key = appKey(w);
      if (!map.has(key)) map.set(key, []);
      map.get(key)!.push(w);
    }
    return [...map.values()];
  });

  const currentGroup = $derived(current ? groups.find((g) => g.includes(current)) : undefined);

  // ---------------- layout ----------------

  /** Largest box with the window's aspect ratio inside `area`, centered. */
  function fit(w: UserAppWindow, area: Box): Box {
    const r = w.rect;
    const valid = r && !w.isIconic && r.right - r.left > 50 && r.bottom - r.top > 50;
    const aspect = valid ? (r.right - r.left) / (r.bottom - r.top) : 16 / 10;
    let width = area.width;
    let height = width / aspect;
    if (height > area.height) {
      height = area.height;
      width = height * aspect;
    }
    return {
      x: area.x + (area.width - width) / 2,
      y: area.y + (area.height - height) / 2,
      width,
      height,
    };
  }

  const MARGIN = 40;
  const CARD_PAD = 10;
  const CARD_HEADER = 34;
  const CARD_GAP = 14;
  /** a covered card still shows at least its header */
  const MIN_VISIBLE_CARD = CARD_HEADER + 14;
  const SMALL_WIDTH = 164;
  const SMALL_HEIGHT = 104;
  const SMALL_GAP = 16;
  const MIN_VISIBLE_SMALL = 40;

  /**
   * Lays `n` cards of `size` along `available` px, like the dock's stacked icons: when
   * they do not fit, each card is covered by the next one, except the focused card and
   * the last one, which stay whole. Too many even for that: only a window of cards around
   * the focused one is shown.
   */
  function stack(n: number, size: number, gap: number, available: number, focus: number, minVisible: number) {
    const fits = n * size + (n - 1) * gap <= available;
    if (fits || n <= 1) {
      return {
        start: 0,
        offsets: Array.from({ length: n }, (_, i) => i * (size + gap)),
        visible: Array.from({ length: n }, () => size),
        total: n * size + Math.max(0, n - 1) * gap,
      };
    }
    const wholeCount = (count: number, f: number) => (f === count - 1 ? 1 : 2);
    let start = 0;
    let count = n;
    let f = focus;
    let whole = wholeCount(count, f);
    let pitch = (available - whole * size) / (count - whole);
    if (pitch < minVisible) {
      count = Math.max(1, Math.min(n, whole + Math.floor((available - whole * size) / minVisible)));
      start = Math.max(0, Math.min(focus - Math.floor(count / 2), n - count));
      f = focus - start;
      whole = wholeCount(count, f);
      pitch = count > whole ? (available - whole * size) / (count - whole) : size;
    }
    const offsets: number[] = [];
    const visible: number[] = [];
    let at = 0;
    for (let i = 0; i < count; i++) {
      const v = i === f || i === count - 1 ? size : Math.min(size, pitch);
      offsets.push(at);
      visible.push(v);
      at += v;
    }
    return { start, offsets, visible, total: at };
  }

  const stripWidth = $derived(Math.max(200, Math.min(300, viewport.width * 0.19)));

  const strip = $derived.by(() => {
    const H = viewport.height;
    const width = stripWidth;
    const thumbWidth = width - CARD_PAD * 2;
    const available = H - MARGIN * 2;
    const g = groups.length;
    // shrink first (down to a readable preview), then stack
    const preferred = Math.round(thumbWidth * 0.62);
    const fitting = (available - (g - 1) * CARD_GAP) / Math.max(1, g) - CARD_PAD - CARD_HEADER;
    const thumbHeight = Math.max(84, Math.min(preferred, fitting));
    const cardHeight = thumbHeight + CARD_PAD + CARD_HEADER;
    const focus = currentGroup ? Math.max(0, groups.indexOf(currentGroup)) : 0;
    const layout = stack(g, cardHeight, CARD_GAP, available, focus, MIN_VISIBLE_CARD);
    const top = (H - layout.total) / 2;
    return layout.offsets.map((offset, i) => {
      const group = groups[layout.start + i]!;
      const card: Box = { x: MARGIN, y: top + offset, width, height: cardHeight };
      const thumb = fit(group[0]!, {
        x: card.x + CARD_PAD,
        y: card.y + CARD_HEADER,
        width: thumbWidth,
        height: thumbHeight,
      });
      // the part of the preview not covered by the next card
      const visibleBottom = card.y + layout.visible[i]!;
      const visibleHeight = Math.max(0, Math.min(1, (visibleBottom - thumb.y) / thumb.height));
      const focused = layout.start + i === focus;
      return { group, card, thumb, visibleHeight, z: focused ? 500 : i + 1 };
    });
  });

  const stage = $derived.by(() => {
    if (!current) return null;
    const W = viewport.width;
    const H = viewport.height;
    const left = MARGIN + stripWidth + 56;
    const right = W - 56;
    const siblings = (currentGroup ?? []).length > 1 ? currentGroup! : [];
    const rowHeight = siblings.length ? SMALL_HEIGHT : 0;
    const area: Box = {
      x: left,
      y: H * 0.08,
      width: right - left,
      height: H * 0.84 - 56 - (rowHeight ? rowHeight + 28 : 0),
    };
    const main = fit(current, area);
    const titleY = main.y + main.height + 16;
    const rowY = titleY + 56;
    const focus = Math.max(0, siblings.indexOf(current));
    const layout = stack(siblings.length, SMALL_WIDTH, SMALL_GAP, right - left, focus, MIN_VISIBLE_SMALL);
    const rowLeft = left + (right - left - layout.total) / 2;
    const small = layout.offsets.map((offset, i) => {
      const w = siblings[layout.start + i]!;
      const frame: Box = { x: rowLeft + offset, y: rowY, width: SMALL_WIDTH, height: rowHeight };
      const thumb = fit(w, { x: frame.x + 6, y: rowY + 6, width: SMALL_WIDTH - 12, height: rowHeight - 12 });
      const visibleRight = frame.x + layout.visible[i]!;
      const visibleWidth = Math.max(0, Math.min(1, (visibleRight - thumb.x) / thumb.width));
      return { window: w, frame, thumb, visibleWidth, z: w === current ? 500 : i + 1 };
    });
    return { main, titleY, small };
  });

  // live previews (minimized windows have no content to show: icon placeholder instead)
  $effect(() => {
    const list: ThumbnailRequest[] = [];
    const add = (key: string, w: UserAppWindow, b: Box, visibleWidth = 1, visibleHeight = 1) => {
      if (!w.isIconic) list.push({ key, hwnd: w.hwnd, ...b, visibleWidth, visibleHeight });
    };
    for (const { group, thumb, visibleHeight } of strip) {
      add(`card-${group[0]!.hwnd}`, group[0]!, thumb, 1, visibleHeight);
    }
    if (stage && current) {
      add("main", current, stage.main);
      for (const s of stage.small) add(`small-${s.window.hwnd}`, s.window, s.thumb, s.visibleWidth, 1);
    }
    api.overlaySetThumbnails(list);
  });

  const px = (b: Box) =>
    `left:${b.x}px;top:${b.y}px;width:${b.width}px;height:${b.height}px`;
</script>

<svelte:window onkeydown={onKeydown} />

<div
  class="switcher"
  role="presentation"
  onclick={(e) => {
    if (e.target === e.currentTarget) api.overlayHide();
  }}
>
  {#if !ordered.length}
    <div class="empty">{t("switcher.empty")}</div>
  {/if}

  {#each strip as { group, card, thumb, visibleHeight, z } (group[0]!.hwnd)}
    {@const w = group[0]!}
    <!-- more windows of the same app: card edges peeking out behind it -->
    {#each group.slice(1, 3) as _, k (k)}
      <div
        class="card-ghost"
        style="{px({
          x: card.x + (k + 1) * 7,
          y: card.y + (k + 1) * 5,
          width: card.width,
          height: card.height - (k + 1) * 10,
        })};z-index:{z}"
      ></div>
    {/each}
    <button
      type="button"
      class="card"
      class:active={currentGroup === group}
      style="{px(card)};z-index:{z}"
      onclick={() => activate(w)}
    >
      <span class="label">
        <img src={iconUrl(w.path, w.umid)} alt="" draggable="false" />
        <span class="text">{w.appName || w.title}</span>
        {#if group.length > 1}<span class="count">{group.length}</span>{/if}
      </span>
      {#if w.isIconic && visibleHeight > 0.3}
        <span
          class="placeholder"
          style="left:{thumb.x - card.x}px;top:{thumb.y - card.y}px;width:{thumb.width}px;height:{thumb.height}px"
        >
          <img src={iconUrl(w.path, w.umid)} alt="" draggable="false" />
        </span>
      {/if}
    </button>
  {/each}

  {#if stage && current}
    <button
      type="button"
      class="main-frame"
      style={px(stage.main)}
      aria-label={current.title}
      onclick={() => activate(current)}
    >
      {#if current.isIconic}
        <span class="placeholder big">
          <img src={iconUrl(current.path, current.umid)} alt="" draggable="false" />
          <span>{t("switcher.minimized")}</span>
        </span>
      {/if}
    </button>
    <div class="title" style:top="{stage.titleY}px" style:left="{stage.main.x}px" style:width="{stage.main.width}px">
      <img src={iconUrl(current.path, current.umid)} alt="" draggable="false" />
      <div class="title-text">
        <span class="window-title">{current.title}</span>
        <span class="app-name">{current.appName}</span>
      </div>
    </div>
    {#each stage.small as s (s.window.hwnd)}
      <button
        type="button"
        class="small"
        class:active={s.window === current}
        style="{px(s.frame)};z-index:{s.z}"
        aria-label={s.window.title}
        onclick={() => activate(s.window)}
      >
        {#if s.window.isIconic}
          <img src={iconUrl(s.window.path, s.window.umid)} alt="" draggable="false" />
        {/if}
      </button>
    {/each}
  {/if}

  <div class="hint">{t("switcher.hint")}</div>
</div>
