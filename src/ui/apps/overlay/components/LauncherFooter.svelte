<!-- Bottom bar of the launcher, like the one of the Windows start menu: the signed-in user
     on the left (lock, sign out, account settings), then the shortcuts chosen in
     Settings > Launcher (settings, downloads...) and the power button (sleep, shut down,
     restart). Shut down, restart and sign out need a second click. -->
<script lang="ts">
  import { onMount } from "svelte";
  import Icon from "@shared/components/Icon.svelte";
  import { t, type TranslationKey } from "@shared/i18n/index.svelte.ts";
  import { api } from "@shared/ipc.ts";
  import { launcherPlace } from "@shared/launcherPlaces.ts";
  import { settingsState } from "@shared/state/settings.svelte.ts";

  type PowerAction = "lock" | "signOut" | "sleep" | "restart" | "shutdown";
  interface Entry {
    id: PowerAction | "account";
    icon: string;
    label: TranslationKey;
    confirm?: boolean;
  }
  const MENUS: Record<"user" | "power", Entry[]> = {
    user: [
      { id: "account", icon: "Settings", label: "launcher.account" },
      { id: "lock", icon: "Lock", label: "power.lock" },
      { id: "signOut", icon: "LogOut", label: "power.signOut", confirm: true },
    ],
    power: [
      { id: "sleep", icon: "Moon", label: "power.sleep" },
      { id: "shutdown", icon: "PowerOff", label: "power.shutdown", confirm: true },
      { id: "restart", icon: "RotateCcw", label: "power.restart", confirm: true },
    ],
  };

  /** which menu is open; the launcher closes it on Esc or a click elsewhere */
  let { open = $bindable(null) }: { open: "user" | "power" | null } = $props();

  const places = $derived(settingsState.value.launcher.places);
  let userName = $state("");
  // destructive actions need a second click
  let armed = $state<string | null>(null);
  let armTimer: ReturnType<typeof setTimeout> | undefined;

  onMount(() => {
    api.getUserName().then((name) => (userName = name));
    return () => clearTimeout(armTimer);
  });

  $effect(() => {
    if (!open) armed = null;
  });

  function toggle(e: MouseEvent, menu: "user" | "power") {
    e.stopPropagation();
    open = open === menu ? null : menu;
  }

  function run(e: MouseEvent, entry: Entry) {
    e.stopPropagation();
    if (entry.confirm && armed !== entry.id) {
      armed = entry.id;
      clearTimeout(armTimer);
      armTimer = setTimeout(() => (armed = null), 3000);
      return;
    }
    open = null;
    if (entry.id === "account") {
      api.launcherOpenPlace("account");
    } else {
      api.overlayHide();
      api.powerAction(entry.id);
    }
  }
</script>

{#snippet menu(kind: "user" | "power")}
  <div class="menu footer-menu {kind}" role="menu">
    {#each MENUS[kind] as entry (entry.id)}
      <button type="button" role="menuitem" class:armed={armed === entry.id} onclick={(e) => run(e, entry)}>
        <Icon name={entry.icon} size={15} />
        {armed === entry.id ? t("power.confirm") : t(entry.label)}
      </button>
    {/each}
  </div>
{/snippet}

<div class="footer" role="toolbar">
  <div class="footer-side">
    <button type="button" class="footer-button user" class:active={open === "user"} onclick={(e) => toggle(e, "user")}>
      <span class="avatar"><Icon name="CircleUserRound" size={22} /></span>
      {#if userName}<span class="user-name">{userName}</span>{/if}
    </button>
    {#if open === "user"}{@render menu("user")}{/if}
  </div>

  <div class="footer-side right">
    {#each places as id (id)}
      {@const place = launcherPlace(id)}
      <button
        type="button"
        class="footer-button"
        title={t(place.label)}
        aria-label={t(place.label)}
        onclick={() => api.launcherOpenPlace(id)}
      >
        <Icon name={place.icon} size={19} />
      </button>
    {/each}
    <button
      type="button"
      class="footer-button"
      class:active={open === "power"}
      title={t("launcher.power")}
      aria-label={t("launcher.power")}
      onclick={(e) => toggle(e, "power")}
    >
      <Icon name="Power" size={19} />
    </button>
    {#if open === "power"}{@render menu("power")}{/if}
  </div>
</div>
