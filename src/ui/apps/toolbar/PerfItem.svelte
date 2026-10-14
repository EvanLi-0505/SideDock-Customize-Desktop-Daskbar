<!-- CPU usage, memory usage or network speed. A click opens Task Manager. Numbers use
     fixed-width digits and a minimum width so the toolbar does not shift as they change. -->
<script lang="ts">
  import Icon from "@shared/components/Icon.svelte";
  import { t } from "@shared/i18n/index.svelte.ts";
  import { api } from "@shared/ipc.ts";
  import { formatGb, formatSpeed } from "@shared/toolbarItems.ts";
  import { tooltip } from "./overlays.ts";
  import { toolbar } from "./state.svelte.ts";

  let { kind, twoLines }: { kind: "cpu" | "memory" | "net-speed"; twoLines: boolean } = $props();

  const perf = $derived(toolbar.system.perf);
  const percent = (value: number | null | undefined) => (value == null ? "--%" : `${value}%`);

  const tip = $derived.by(() => {
    if (kind === "cpu") return t("perf.cpuTip", { value: percent(perf?.cpu) });
    if (kind === "memory") {
      const m = perf?.memory;
      return m
        ? t("perf.memoryTip", { used: formatGb(m.used), total: formatGb(m.total), value: `${m.percent}%` })
        : t("toolbarItem.memory");
    }
    const n = perf?.net;
    return n ? t("perf.netTip", { down: formatSpeed(n.down), up: formatSpeed(n.up) }) : t("toolbarItem.net-speed");
  });
</script>

<button
  type="button"
  class="tb-button perf"
  class:net={kind === "net-speed"}
  class:two-lines={twoLines}
  use:tooltip={() => tip}
  onclick={() => api.shellAction("task-manager")}
>
  {#if kind === "cpu"}
    <Icon name="Cpu" />
    <span class="perf-value">{percent(perf?.cpu)}</span>
  {:else if kind === "memory"}
    <Icon name="MemoryStick" />
    <span class="perf-value">{percent(perf?.memory?.percent)}</span>
  {:else}
    <span class="net-lines">
      <span class="net-line"><Icon name="ArrowDown" size={11} />{perf?.net ? formatSpeed(perf.net.down) : "--"}</span>
      <span class="net-line"><Icon name="ArrowUp" size={11} />{perf?.net ? formatSpeed(perf.net.up) : "--"}</span>
    </span>
  {/if}
</button>
