<script lang="ts">
  import { t, type TranslationKey } from "@shared/i18n/index.svelte.ts";
  import Sidebar from "./components/Sidebar.svelte";
  import Titlebar from "./components/Titlebar.svelte";
  import { PAGES, type PageId } from "./pages.ts";

  let page = $state<PageId>("home");
  const current = $derived(PAGES.find((p) => p.id === page)!);
</script>

<div class="app">
  <Titlebar title={t(current.label as TranslationKey)} />
  <div class="app-body">
    <Sidebar active={page} onselect={(id) => (page = id)} />
    <main class="content">
      {#key page}
        <div class="page">
          <current.component navigate={(id: PageId) => (page = id)} />
        </div>
      {/key}
    </main>
  </div>
</div>
