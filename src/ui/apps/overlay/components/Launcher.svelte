<!-- macOS Launchpad-like start screen: search on top, paged grid of every Start menu app.
     Typing searches (pinyin and initials too), arrows move the selection, Enter starts,
     Esc clears the search then closes.

     Paging is native horizontal scrolling with one snap point per page (touchpad swipes,
     momentum and snapping are the browser's, never several pages per swipe). Vertical
     wheel / touchpad input is turned into one page per gesture (see onWheel). -->
<script lang="ts">
  import { onMount, untrack } from "svelte";
  import Icon from "@shared/components/Icon.svelte";
  import { t } from "@shared/i18n/index.svelte.ts";
  import { api, iconUrl } from "@shared/ipc.ts";
  import { settingsState } from "@shared/state/settings.svelte.ts";
  import type { StartApp } from "@shared/types.ts";
  import { search, startApps } from "../state/apps.svelte.ts";
  import { wheelStepper } from "../wheel.ts";

  let query = $state("");
  let selected = $state(-1);
  let page = $state(0);
  let input: HTMLInputElement | null = $state(null);
  let scroller: HTMLDivElement | null = $state(null);
  let root: HTMLDivElement | null = $state(null);
  let gridWidth = $state(0);
  let gridHeight = $state(0);
  let menu = $state<{ app: StartApp; x: number; y: number } | null>(null);
  let notice = $state("");

  const iconSize = $derived(settingsState.value.launcher.iconSize);
  const cellWidth = $derived(iconSize + 64);
  const cellHeight = $derived(iconSize + 58);
  const columns = $derived(Math.max(3, Math.min(9, Math.floor(gridWidth / cellWidth) || 1)));
  const rows = $derived(Math.max(2, Math.min(6, Math.floor(gridHeight / cellHeight) || 1)));
  const perPage = $derived(columns * rows);

  const results = $derived(search(startApps.list, query));
  const pages = $derived.by(() => {
    const out: StartApp[][] = [];
    for (let i = 0; i < results.length; i += perPage) out.push(results.slice(i, i + perPage));
    return out.length ? out : [[]];
  });

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
      selected = Math.min(results.length - 1, next * perPage);
    }
  }

  // a new search starts on its first page, on its best match
  $effect(() => {
    void query;
    untrack(() => {
      selected = query.trim() ? 0 : -1;
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
    };
  });

  function launch(app: StartApp, elevated = false) {
    api.overlayLaunch(app.id, elevated);
  }

  function move(delta: number) {
    if (!results.length) return;
    const from = selected < 0 ? page * perPage - (delta > 0 ? 1 : 0) : selected;
    selected = Math.max(0, Math.min(results.length - 1, from + delta));
  }

  function turn(delta: number) {
    const next = Math.max(0, Math.min(pages.length - 1, page + delta));
    if (next === page) return;
    if (selected >= 0) selected = Math.min(results.length - 1, next * perPage);
    goTo(next);
  }

  function onKeydown(e: KeyboardEvent) {
    if (menu) {
      if (e.key === "Escape") menu = null;
      return;
    }
    switch (e.key) {
      case "Escape":
        e.preventDefault();
        if (query) query = "";
        else api.overlayHide();
        break;
      case "Enter": {
        const app = results[selected] ?? (query.trim() ? results[0] : undefined);
        if (app) launch(app, e.ctrlKey && e.shiftKey);
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
    const horizontal = Math.abs(e.deltaX) > Math.abs(e.deltaY);
    // horizontal swipes over the grid: native scrolling + snapping
    if (horizontal && scroller?.contains(e.target as Node)) return;
    e.preventDefault();
    stepper.handle(e, horizontal ? e.deltaX : e.deltaY);
  }

  function openMenu(e: MouseEvent, app: StartApp) {
    e.preventDefault();
    menu = {
      app,
      x: Math.min(e.clientX, window.innerWidth - 230),
      y: Math.min(e.clientY, window.innerHeight - 190),
    };
  }

  async function pin(app: StartApp) {
    menu = null;
    try {
      const added = await api.pinStartApp(app.id);
      notice = added ? t("launcher.pinned", { name: app.name }) : t("launcher.alreadyPinned");
    } catch (err) {
      notice = String(err);
    }
    setTimeout(() => (notice = ""), 1800);
  }

  function reveal(app: StartApp) {
    menu = null;
    if (app.path) api.revealPath(app.path);
    api.overlayHide();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<!-- clicking anywhere that is not an app or the search closes the launcher -->
<div
  class="launcher"
  role="presentation"
  bind:this={root}
  onclick={(e) => {
    if (menu) menu = null;
    else if (e.target === e.currentTarget) api.overlayHide();
  }}
>
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

  <div
    class="grid-area"
    role="presentation"
    bind:clientWidth={gridWidth}
    bind:clientHeight={gridHeight}
    onclick={(e) => {
      if (e.target === e.currentTarget) api.overlayHide();
    }}
  >
    {#if !results.length}
      <div class="empty">{startApps.list.length ? t("launcher.noResults") : t("launcher.loading")}</div>
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
            {#each items as app, i (app.id)}
              {@const index = p * perPage + i}
              <button
                type="button"
                class="app"
                class:selected={index === selected}
                title={app.name}
                onclick={() => launch(app)}
                oncontextmenu={(e) => openMenu(e, app)}
                onmouseenter={() => {
                  if (selected >= 0) selected = index;
                }}
              >
                <img
                  src={iconUrl(null, app.id)}
                  alt=""
                  loading="lazy"
                  decoding="async"
                  draggable="false"
                  style:width="{iconSize}px"
                  style:height="{iconSize}px"
                />
                <span class="name">{app.name}</span>
              </button>
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

  {#if notice}
    <div class="notice">{notice}</div>
  {/if}

  {#if menu}
    {@const app = menu.app}
    <div class="menu" style:left="{menu.x}px" style:top="{menu.y}px" role="menu">
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
    </div>
  {/if}
</div>
