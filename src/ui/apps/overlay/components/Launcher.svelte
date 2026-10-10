<!-- macOS Launchpad-like start screen: search on top, paged grid of apps and collections.

     Home shows the user's arrangement (state/layout.svelte.ts): apps and collections
     (folders), followed by every other app unless Settings > Launcher limits it to the
     added ones. "All apps" lists everything alphabetically. Typing searches every app
     (pinyin and initials too), arrows move the selection, Enter starts, Esc steps back.

     On the home grid tiles are dragged to reorder them, onto another app to create a
     collection, onto a collection to add to it, and out of an open collection.

     Paging is native horizontal scrolling with one snap point per page (touchpad swipes,
     momentum and snapping are the browser's, never several pages per swipe). Vertical
     wheel / touchpad input is turned into one page per gesture (see onWheel). -->
<script lang="ts">
  import { onMount, tick, untrack } from "svelte";
  import Icon from "@shared/components/Icon.svelte";
  import { t } from "@shared/i18n/index.svelte.ts";
  import { api, iconUrl } from "@shared/ipc.ts";
  import { settingsState } from "@shared/state/settings.svelte.ts";
  import { insertionIndex } from "@shared/trayDrag.ts";
  import type { StartApp } from "@shared/types.ts";
  import { search, startApps } from "../state/apps.svelte.ts";
  import { launcherLayout, layoutActions, type Tile } from "../state/layout.svelte.ts";
  import { wheelStepper } from "../wheel.ts";

  type FolderTile = Extract<Tile, { kind: "folder" }>;

  let query = $state("");
  let view = $state<"home" | "all">("home");
  let selected = $state(-1);
  let page = $state(0);
  let input: HTMLInputElement | null = $state(null);
  let scroller: HTMLDivElement | null = $state(null);
  let root: HTMLDivElement | null = $state(null);
  let gridWidth = $state(0);
  let gridHeight = $state(0);
  let notice = $state("");

  const iconSize = $derived(settingsState.value.launcher.iconSize);
  const cellWidth = $derived(iconSize + 64);
  const cellHeight = $derived(iconSize + 58);
  const columns = $derived(Math.max(3, Math.min(9, Math.floor(gridWidth / cellWidth) || 1)));
  const rows = $derived(Math.max(2, Math.min(6, Math.floor(gridHeight / cellHeight) || 1)));
  const perPage = $derived(columns * rows);

  const searching = $derived(query.trim().length > 0);
  const appTile = (app: StartApp): Tile => ({ kind: "app", id: app.id, app });
  const tiles = $derived<Tile[]>(
    searching
      ? search(startApps.list, query).map(appTile)
      : view === "all"
        ? startApps.list.map(appTile)
        : launcherLayout.tiles,
  );
  /** the arrangement can be edited (drag, collections) */
  const editable = $derived(!searching && view === "home");
  const pages = $derived.by(() => {
    const out: Tile[][] = [];
    for (let i = 0; i < tiles.length; i += perPage) out.push(tiles.slice(i, i + perPage));
    return out.length ? out : [[]];
  });

  // ---------------- collections ----------------
  let openFolderId = $state<string | null>(null);
  const openFolder = $derived(
    openFolderId ? launcherLayout.tiles.find((x): x is FolderTile => x.kind === "folder" && x.id === openFolderId) : undefined,
  );
  let panelEl: HTMLDivElement | null = $state(null);
  let renaming = $state(false);
  let renameValue = $state("");
  let renameInput: HTMLInputElement | null = $state(null);

  async function startRename(folder: FolderTile) {
    openFolderId = folder.id;
    renameValue = folder.name;
    renaming = true;
    await tick();
    renameInput?.select();
  }

  function commitRename() {
    if (renaming && openFolderId) layoutActions.rename(openFolderId, renameValue);
    renaming = false;
  }

  function closeFolder() {
    commitRename();
    openFolderId = null;
  }

  // ---------------- paging & selection ----------------

  /** Scrolls to a page; `page` itself follows the scroll position (onScroll). */
  function goTo(target: number, smooth = true) {
    if (!scroller) return;
    const clamped = Math.max(0, Math.min(pages.length - 1, target));
    scroller.scrollTo({ left: clamped * scroller.clientWidth, behavior: smooth ? "smooth" : "instant" });
  }

  function onScroll() {
    if (!scroller || !scroller.clientWidth) return;
    const next = Math.round(scroller.scrollLeft / scroller.clientWidth);
    if (next === page) return;
    page = next;
    // a swipe carries the keyboard selection along
    if (selected >= 0 && Math.floor(selected / perPage) !== next) {
      selected = Math.min(tiles.length - 1, next * perPage);
    }
  }

  // a new search or view starts on its first page, a search on its best match
  $effect(() => {
    void query;
    void view;
    untrack(() => {
      selected = searching ? 0 : -1;
      goTo(0, false);
    });
  });

  // keep the selection on screen (arrow keys)
  $effect(() => {
    if (selected < 0) return;
    const target = Math.floor(selected / perPage);
    untrack(() => {
      if (target !== page) goTo(target);
    });
  });

  // new grid size: stay on the same page
  $effect(() => {
    void gridWidth;
    void perPage;
    untrack(() => goTo(page, false));
  });

  onMount(() => {
    input?.focus();
    // not passive: vertical input must not scroll anything natively
    root?.addEventListener("wheel", onWheel, { passive: false });
    return () => {
      root?.removeEventListener("wheel", onWheel);
      stepper.dispose();
      clearTimeout(edgeTimer);
    };
  });

  function launch(app: StartApp, elevated = false) {
    api.overlayLaunch(app.id, elevated);
  }

  function activate(tile: Tile, elevated = false) {
    if (tile.kind === "app") launch(tile.app, elevated);
    else openFolderId = tile.id;
  }

  function move(delta: number) {
    if (!tiles.length) return;
    const from = selected < 0 ? page * perPage - (delta > 0 ? 1 : 0) : selected;
    selected = Math.max(0, Math.min(tiles.length - 1, from + delta));
  }

  function turn(delta: number) {
    const next = Math.max(0, Math.min(pages.length - 1, page + delta));
    if (next === page) return;
    if (selected >= 0) selected = Math.min(tiles.length - 1, next * perPage);
    goTo(next);
  }

  function onKeydown(e: KeyboardEvent) {
    if (menu) {
      if (e.key === "Escape") menu = null;
      return;
    }
    if (renaming) {
      if (e.key === "Enter") commitRename();
      else if (e.key === "Escape") renaming = false;
      return;
    }
    if (openFolder) {
      if (e.key === "Escape") closeFolder();
      return;
    }
    switch (e.key) {
      case "Escape":
        e.preventDefault();
        if (query) query = "";
        else if (view === "all") view = "home";
        else api.overlayHide();
        break;
      case "Enter": {
        const tile = tiles[selected] ?? (searching ? tiles[0] : undefined);
        if (tile) activate(tile, e.ctrlKey && e.shiftKey);
        break;
      }
      case "ArrowRight":
        e.preventDefault();
        move(1);
        break;
      case "ArrowLeft":
        e.preventDefault();
        move(-1);
        break;
      case "ArrowDown":
        e.preventDefault();
        move(columns);
        break;
      case "ArrowUp":
        e.preventDefault();
        move(-columns);
        break;
      case "PageDown":
        e.preventDefault();
        turn(1);
        break;
      case "PageUp":
        e.preventDefault();
        turn(-1);
        break;
      default:
        // typing anywhere goes to the search field
        if (input && document.activeElement !== input && e.key.length === 1) input.focus();
    }
  }

  // ---------------- wheel & touchpad ----------------
  const stepper = wheelStepper((direction) => turn(direction));

  function onWheel(e: WheelEvent) {
    if (menu || e.ctrlKey) return;
    // an open collection scrolls by itself
    if (openFolder) return;
    const horizontal = Math.abs(e.deltaX) > Math.abs(e.deltaY);
    // horizontal swipes over the grid: native scrolling + snapping
    if (horizontal && scroller?.contains(e.target as Node)) return;
    e.preventDefault();
    stepper.handle(e, horizontal ? e.deltaX : e.deltaY);
  }

  // ---------------- context menu ----------------
  type Menu = { x: number; y: number } & ({ app: StartApp; folderId: string | null } | { folder: FolderTile });
  let menu = $state<Menu | null>(null);

  function openMenu(e: MouseEvent, target: { app: StartApp; folderId: string | null } | { folder: FolderTile }) {
    e.preventDefault();
    e.stopPropagation();
    menu = {
      ...target,
      x: Math.min(e.clientX, window.innerWidth - 240),
      y: Math.min(e.clientY, window.innerHeight - 300),
    };
  }

  function flash(text: string) {
    notice = text;
    setTimeout(() => (notice = ""), 1800);
  }

  async function pin(app: StartApp) {
    menu = null;
    try {
      const added = await api.pinStartApp(app.id);
      flash(added ? t("launcher.pinned", { name: app.name }) : t("launcher.alreadyPinned"));
    } catch (err) {
      flash(String(err));
    }
  }

  function reveal(app: StartApp) {
    menu = null;
    if (app.path) api.revealPath(app.path);
    api.overlayHide();
  }

  function newFolder(app: StartApp) {
    menu = null;
    const id = layoutActions.newFolder(app.id);
    // straight to naming it
    tick().then(() => {
      const folder = launcherLayout.tiles.find((x): x is FolderTile => x.kind === "folder" && x.id === id);
      if (folder) startRename(folder);
    });
  }

  function removeFromFolder(app: StartApp) {
    menu = null;
    // back on the grid, right after its collection
    const at = launcherLayout.tiles.findIndex((x) => x.kind === "folder" && x.apps.some((a) => a.id === app.id));
    layoutActions.moveOutOfFolder(app.id, at + 1);
  }

  function setShown(app: StartApp, shown: boolean) {
    menu = null;
    if (shown) {
      layoutActions.add(app.id);
      flash(t("launcher.added", { name: app.name }));
    } else {
      layoutActions.remove(app.id);
      flash(t("launcher.removed", { name: app.name }));
    }
  }

  // ---------------- drag & drop (home grid) ----------------
  const DRAG_THRESHOLD = 6;
  const EDGE = 56;

  interface Marker {
    left: number;
    top: number;
    height: number;
  }
  type DropTarget =
    | { type: "insert"; index: number; marker: Marker | null }
    | { type: "merge"; id: string }
    | { type: "folder"; index: number; marker: Marker | null };

  let press: { tile: Tile; fromFolder: string | null; x: number; y: number; pointer: number; el: HTMLElement } | null =
    null;
  let drag = $state<{ tile: Tile; fromFolder: string | null; x: number; y: number } | null>(null);
  let target = $state<DropTarget | null>(null);
  let suppressClick = false;
  let edgeTimer = 0;
  let edgeDirection = 0;

  function pressTile(e: PointerEvent, tile: Tile, fromFolder: string | null) {
    if (!editable || e.button !== 0 || e.pointerType === "touch") return;
    press = { tile, fromFolder, x: e.clientX, y: e.clientY, pointer: e.pointerId, el: e.currentTarget as HTMLElement };
  }

  const tilesIn = (container: Element, exceptId: string) =>
    Array.from(container.querySelectorAll<HTMLElement>("[data-id]")).filter((el) => el.dataset.id !== exceptId);

  function markerFor(els: HTMLElement[], index: number): Marker | null {
    const el = els[Math.min(index, els.length - 1)];
    if (!el) return null;
    const r = el.getBoundingClientRect();
    return { left: index < els.length ? r.left : r.right, top: r.top + 6, height: iconSize + 8 };
  }

  function updateTarget(x: number, y: number) {
    if (!drag) return;
    const id = drag.tile.id;
    // reordering inside the open collection, until the pointer leaves it
    if (openFolder && drag.fromFolder === openFolder.id && panelEl) {
      const r = panelEl.getBoundingClientRect();
      if (x >= r.left && x <= r.right && y >= r.top && y <= r.bottom) {
        const els = tilesIn(panelEl, id);
        const index = insertionIndex(els, x, y);
        target = { type: "folder", index, marker: markerFor(els, index) };
        return;
      }
      closeFolder();
    }
    const pageEl = scroller?.children[page];
    if (!pageEl) {
      target = null;
      return;
    }
    const els = tilesIn(pageEl, id);
    const others = tiles.filter((x) => x.id !== id);
    const indexOf = (el: HTMLElement) => others.findIndex((x) => x.id === el.dataset.id);
    for (const el of els) {
      const r = el.getBoundingClientRect();
      if (x < r.left || x > r.right || y < r.top || y > r.bottom) continue;
      const center = r.left + r.width / 2;
      // the middle of a tile: into (or a new) collection
      if (drag.tile.kind === "app" && Math.abs(x - center) < r.width * 0.26) {
        target = { type: "merge", id: el.dataset.id! };
        return;
      }
      const after = x > center;
      target = {
        type: "insert",
        index: indexOf(el) + (after ? 1 : 0),
        marker: { left: after ? r.right : r.left, top: r.top + 6, height: iconSize + 8 },
      };
      return;
    }
    const local = insertionIndex(els, x, y);
    const index = !els.length
      ? Math.min(others.length, page * perPage)
      : local < els.length
        ? indexOf(els[local]!)
        : indexOf(els[els.length - 1]!) + 1;
    target = { type: "insert", index, marker: markerFor(els, local) };
  }

  /** holding a tile at the left / right edge turns the page */
  function edgePaging(x: number) {
    const direction = x < EDGE ? -1 : x > window.innerWidth - EDGE ? 1 : 0;
    if (direction === edgeDirection) return;
    edgeDirection = direction;
    clearTimeout(edgeTimer);
    if (!direction || openFolder) return;
    const step = () => {
      turn(direction);
      target = null;
      edgeTimer = window.setTimeout(step, 900);
    };
    edgeTimer = window.setTimeout(step, 550);
  }

  function onPointerMove(e: PointerEvent) {
    if (!press || e.pointerId !== press.pointer) return;
    if (!drag) {
      if (Math.hypot(e.clientX - press.x, e.clientY - press.y) < DRAG_THRESHOLD) return;
      press.el.setPointerCapture(e.pointerId);
      menu = null;
    }
    drag = { tile: press.tile, fromFolder: press.fromFolder, x: e.clientX, y: e.clientY };
    updateTarget(e.clientX, e.clientY);
    edgePaging(e.clientX);
  }

  function onPointerUp(e: PointerEvent) {
    if (!press || e.pointerId !== press.pointer) return;
    press = null;
    clearTimeout(edgeTimer);
    edgeDirection = 0;
    if (!drag) return;
    const { tile, fromFolder } = drag;
    const drop = e.type === "pointerup" ? target : null;
    drag = null;
    target = null;
    // the click that follows the release must not start the app
    suppressClick = true;
    setTimeout(() => (suppressClick = false), 0);
    if (!drop) return;
    if (drop.type === "folder" && fromFolder) layoutActions.moveInFolder(fromFolder, tile.id, drop.index);
    else if (drop.type === "merge") layoutActions.merge(tile.id, drop.id);
    else if (drop.type === "insert") {
      if (fromFolder) layoutActions.moveOutOfFolder(tile.id, drop.index);
      else layoutActions.move(tile.id, drop.index);
    }
  }

  function clickTile(tile: Tile) {
    if (suppressClick) return;
    activate(tile);
  }
</script>

<svelte:window
  onkeydown={onKeydown}
  onpointermove={onPointerMove}
  onpointerup={onPointerUp}
  onpointercancel={onPointerUp}
/>

{#snippet folderIcon(folder: FolderTile, size: number)}
  <span class="folder-icon" style:width="{size}px" style:height="{size}px">
    {#each folder.apps.slice(0, 4) as app (app.id)}
      <img src={iconUrl(null, app.id)} alt="" draggable="false" />
    {/each}
  </span>
{/snippet}

{#snippet tileButton(tile: Tile, index: number, fromFolder: string | null)}
  <button
    type="button"
    class="app"
    class:selected={index >= 0 && index === selected}
    class:dragging={drag?.tile.id === tile.id}
    class:merge={target?.type === "merge" && target.id === tile.id}
    data-id={tile.id}
    title={tile.kind === "app" ? tile.app.name : tile.name}
    onclick={() => clickTile(tile)}
    onpointerdown={(e) => pressTile(e, tile, fromFolder)}
    oncontextmenu={(e) =>
      openMenu(e, tile.kind === "app" ? { app: tile.app, folderId: fromFolder } : { folder: tile })}
    onmouseenter={() => {
      if (selected >= 0 && index >= 0) selected = index;
    }}
  >
    {#if tile.kind === "app"}
      <img
        src={iconUrl(null, tile.app.id)}
        alt=""
        loading="lazy"
        decoding="async"
        draggable="false"
        style:width="{iconSize}px"
        style:height="{iconSize}px"
      />
      <span class="name">{tile.app.name}</span>
    {:else}
      {@render folderIcon(tile, iconSize)}
      <span class="name">{tile.name}</span>
    {/if}
  </button>
{/snippet}

<!-- clicking anywhere that is not an app or the search closes the launcher -->
<div
  class="launcher"
  role="presentation"
  bind:this={root}
  onclick={(e) => {
    if (menu) menu = null;
    else if (e.target === e.currentTarget) api.overlayHide();
  }}
  oncontextmenu={() => (menu = null)}
>
  <div class="top">
    <div class="search">
      <Icon name="Search" size={16} />
      <input
        bind:this={input}
        bind:value={query}
        type="text"
        spellcheck="false"
        autocomplete="off"
        placeholder={t("launcher.search")}
      />
    </div>
    {#if !searching}
      <button type="button" class="view-toggle" onclick={() => (view = view === "all" ? "home" : "all")}>
        {#if view === "all"}
          <Icon name="ArrowLeft" size={14} />
          {t("launcher.back")}
        {:else}
          {t("launcher.allApps")}
          <Icon name="ChevronRight" size={14} />
        {/if}
      </button>
    {/if}
  </div>

  <div
    class="grid-area"
    role="presentation"
    bind:clientWidth={gridWidth}
    bind:clientHeight={gridHeight}
    onclick={(e) => {
      if (e.target === e.currentTarget) api.overlayHide();
    }}
  >
    {#if !tiles.length}
      <div class="empty">
        {#if !startApps.list.length}
          {t("launcher.loading")}
        {:else if searching}
          {t("launcher.noResults")}
        {:else}
          {t("launcher.emptyHome")}
        {/if}
      </div>
    {:else}
      <div class="pages" bind:this={scroller} onscroll={onScroll}>
        {#each pages as items, p (p)}
          <div
            class="page"
            role="presentation"
            style:grid-template-columns="repeat({columns}, {cellWidth}px)"
            style:grid-template-rows="repeat({rows}, {cellHeight}px)"
            onclick={(e) => {
              if (e.target === e.currentTarget) api.overlayHide();
            }}
          >
            {#each items as tile, i (tile.id)}
              {@render tileButton(tile, p * perPage + i, null)}
            {/each}
          </div>
        {/each}
      </div>
    {/if}
  </div>

  {#if pages.length > 1}
    <div class="dots">
      {#each pages as _, p (p)}
        <button
          type="button"
          class="dot"
          class:active={p === page}
          aria-label={`${p + 1}`}
          onclick={() => turn(p - page)}
        ></button>
      {/each}
    </div>
  {/if}

  {#if openFolder}
    {@const folder = openFolder}
    <div
      class="folder-backdrop"
      role="presentation"
      onclick={(e) => {
        if (e.target === e.currentTarget) closeFolder();
      }}
    >
      <div class="folder-panel" bind:this={panelEl}>
        {#if renaming}
          <input
            class="folder-title"
            bind:this={renameInput}
            bind:value={renameValue}
            maxlength="64"
            spellcheck="false"
            onblur={commitRename}
          />
        {:else}
          <button type="button" class="folder-title" title={t("launcher.rename")} onclick={() => startRename(folder)}>
            {folder.name}
          </button>
        {/if}
        <div
          class="folder-grid"
          style:grid-template-columns="repeat({Math.min(5, Math.max(3, folder.apps.length))}, {cellWidth}px)"
          style:grid-auto-rows="{cellHeight}px"
        >
          {#each folder.apps as app (app.id)}
            {@render tileButton(appTile(app), -1, folder.id)}
          {/each}
        </div>
      </div>
    </div>
  {/if}

  {#if target && target.type !== "merge" && target.marker}
    <div
      class="drop-marker"
      style:left="{target.marker.left}px"
      style:top="{target.marker.top}px"
      style:height="{target.marker.height}px"
    ></div>
  {/if}

  {#if drag}
    <div class="drag-ghost" style:left="{drag.x}px" style:top="{drag.y}px">
      {#if drag.tile.kind === "app"}
        <img src={iconUrl(null, drag.tile.app.id)} alt="" style:width="{iconSize}px" style:height="{iconSize}px" />
      {:else}
        {@render folderIcon(drag.tile, iconSize)}
      {/if}
    </div>
  {/if}

  {#if notice}
    <div class="notice">{notice}</div>
  {/if}

  {#if menu}
    <div class="menu" style:left="{menu.x}px" style:top="{menu.y}px" role="menu">
      {#if "folder" in menu}
        {@const folder = menu.folder}
        <button
          type="button"
          role="menuitem"
          onclick={() => {
            menu = null;
            openFolderId = folder.id;
          }}
        >
          <Icon name="FolderOpen" size={15} />
          {t("launcher.open")}
        </button>
        <button
          type="button"
          role="menuitem"
          onclick={() => {
            menu = null;
            startRename(folder);
          }}
        >
          <Icon name="Pencil" size={15} />
          {t("launcher.rename")}
        </button>
        <button
          type="button"
          role="menuitem"
          onclick={() => {
            menu = null;
            layoutActions.ungroup(folder.id);
          }}
        >
          <Icon name="X" size={15} />
          {t("launcher.ungroup")}
        </button>
      {:else}
        {@const app = menu.app}
        {@const inFolder = menu.folderId}
        <button type="button" role="menuitem" onclick={() => launch(app)}>
          <Icon name="ExternalLink" size={15} />
          {t("launcher.open")}
        </button>
        {#if !app.packaged && app.path}
          <button type="button" role="menuitem" onclick={() => launch(app, true)}>
            <Icon name="ShieldCheck" size={15} />
            {t("launcher.runAs")}
          </button>
        {/if}
        <button type="button" role="menuitem" onclick={() => pin(app)}>
          <Icon name="Pin" size={15} />
          {t("launcher.pin")}
        </button>
        {#if app.path}
          <button type="button" role="menuitem" onclick={() => reveal(app)}>
            <Icon name="FolderOpen" size={15} />
            {t("launcher.openLocation")}
          </button>
        {/if}
        <hr />
        {#if inFolder}
          <button type="button" role="menuitem" onclick={() => removeFromFolder(app)}>
            <Icon name="LogOut" size={15} />
            {t("launcher.removeFromFolder")}
          </button>
        {:else if editable}
          <button type="button" role="menuitem" onclick={() => newFolder(app)}>
            <Icon name="FolderPlus" size={15} />
            {t("launcher.newCollection")}
          </button>
        {/if}
        {#if launcherLayout.isShown(app.id)}
          <button type="button" role="menuitem" onclick={() => setShown(app, false)}>
            <Icon name="EyeOff" size={15} />
            {t("launcher.remove")}
          </button>
        {:else}
          <button type="button" role="menuitem" onclick={() => setShown(app, true)}>
            <Icon name="Eye" size={15} />
            {t("launcher.add")}
          </button>
        {/if}
      {/if}
    </div>
  {/if}
</div>
