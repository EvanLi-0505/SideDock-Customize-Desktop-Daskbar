// Settings editing: changes apply immediately (debounced) and "Cancel" restores the
// values the window was opened with.

import { untrack } from "svelte";
import { api } from "@shared/ipc.ts";
import { settingsState } from "@shared/state/settings.svelte.ts";
import type { AppSettings } from "@shared/types.ts";
import { clone, debounce } from "@shared/utils.ts";

let local = $state<AppSettings>(clone(settingsState.value));
let initial = $state<AppSettings>(clone(settingsState.value));
/** local holds edits the backend has not confirmed yet */
let pending = false;
let saveError = $state<string | null>(null);

const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);

const persist = debounce(async () => {
  const sent = $state.snapshot(local) as AppSettings;
  try {
    const saved = await api.saveSettings(sent);
    saveError = null;
    // if the user kept editing while saving, keep their newer edits (another save is queued)
    if (same($state.snapshot(local), sent)) {
      local = saved;
      pending = false;
    }
  } catch (err) {
    saveError = String(err);
    pending = false;
    local = clone(settingsState.value);
  }
}, 250);

// Follow changes made elsewhere (tray, dock menu). Only the backend value is a
// dependency: tracking `local` too would make this effect undo every fresh edit
// before it is saved.
$effect.root(() => {
  $effect(() => {
    const remote = settingsState.value;
    untrack(() => {
      if (!pending && !same(remote, $state.snapshot(local))) {
        local = clone(remote);
      }
    });
  });
});

export const editor = {
  get value(): AppSettings {
    return local;
  },
  get dirty(): boolean {
    return !same(local, initial);
  },
  get error(): string | null {
    return saveError;
  },
  update(mutate: (draft: AppSettings) => void) {
    pending = true;
    mutate(local);
    persist();
  },
  revert() {
    pending = true;
    local = clone($state.snapshot(initial) as AppSettings);
    persist();
    persist.flush();
  },
  /** accepts the current values as the new baseline */
  commit() {
    persist.flush();
    initial = clone($state.snapshot(local) as AppSettings);
  },
  async resetSection(section: "dock" | "all") {
    persist.cancel();
    pending = false;
    local = await api.resetSettings(section);
  },
};
