<!-- A button the user added in Settings > Toolbar: opens a program, file, folder or web
     address, runs a command, or opens a key in Registry Editor. Optionally asks first;
     tells what went wrong next to the button. -->
<script lang="ts">
  import { ask } from "@tauri-apps/plugin-dialog";
  import Icon from "@shared/components/Icon.svelte";
  import { t } from "@shared/i18n/index.svelte.ts";
  import { api, iconUrl } from "@shared/ipc.ts";
  import { customButtonIcon, isUrlTarget } from "@shared/toolbarItems.ts";
  import type { CustomButton } from "@shared/types.ts";
  import { openPopup, tooltip } from "./overlays.ts";

  let { button }: { button: CustomButton } = $props();

  let el: HTMLButtonElement | null = $state(null);
  const label = $derived(button.name || button.target || t("custom.kind.command"));
  /** the target's own icon: programs, files and folders without a chosen icon */
  const ownIcon = $derived(!button.icon && button.kind === "open" && !isUrlTarget(button.target));

  const ERRORS: Record<string, Parameters<typeof t>[0]> = {
    "regedit-open": "custom.err.regeditOpen",
    "not-a-registry-path": "custom.err.notRegistryPath",
  };

  async function run() {
    if (button.confirm) {
      const yes = await ask(t("custom.confirmRun", { name: label }), { title: "SideDock", kind: "warning" });
      if (!yes) return;
    }
    try {
      await api.toolbarRunCustom(button.id);
    } catch (err) {
      const code = String(err);
      const key = Object.keys(ERRORS).find((k) => code.includes(k));
      const text = key ? t(ERRORS[key]!) : t("custom.err.failed", { error: code });
      if (el) openPopup(el, "notice", { text });
    }
  }
</script>

<button
  type="button"
  class="tb-button custom"
  aria-label={label}
  bind:this={el}
  use:tooltip={() => label}
  onclick={run}
>
  {#if ownIcon}
    <img src={iconUrl(button.target)} alt="" draggable="false" />
  {:else}
    <Icon name={customButtonIcon(button)} />
  {/if}
</button>
