<!-- Custom toolbar buttons: each opens a program, file, folder or web address. A new button
     is placed right after the start button; drag it elsewhere in the items editor. -->
<script lang="ts">
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import Icon from "@shared/components/Icon.svelte";
  import { t } from "@shared/i18n/index.svelte.ts";
  import { iconUrl } from "@shared/ipc.ts";
  import { customItemId } from "@shared/toolbarItems.ts";
  import type { CustomButton } from "@shared/types.ts";
  import { editor } from "../state/editor.svelte.ts";

  const MAX = 24;
  const ICONS = [
    "MousePointerClick", "AppWindow", "Globe", "Link", "Folder", "FileText", "Terminal", "Code",
    "Rocket", "Star", "Heart", "Bookmark", "Mail", "Music", "Video", "Image", "Camera", "Gamepad2",
    "Calculator", "Cloud", "BookOpen", "Briefcase", "Settings", "Download",
  ];

  const buttons = $derived(editor.value.toolbar.customButtons);

  /** the button being edited (a copy), `null` = no form open */
  let draft = $state<CustomButton | null>(null);
  let isNew = $state(false);
  let error = $state("");

  const isUrl = (target: string) => /^[a-z][a-z0-9+.-]*:(\/\/)?/i.test(target) && !/^[a-z]:\\/i.test(target);

  function startAdd() {
    draft = { id: crypto.randomUUID(), name: "", target: "", args: "", icon: "" };
    isNew = true;
    error = "";
  }

  function startEdit(button: CustomButton) {
    draft = { ...button };
    isNew = false;
    error = "";
  }

  async function browse(directory: boolean) {
    const picked = await openDialog({ title: t("custom.target"), multiple: false, directory });
    if (typeof picked === "string" && draft) {
      draft.target = picked;
      // a sensible default name: the file or folder name without its extension
      if (!draft.name.trim()) {
        const base = picked.split(/[\\/]/).filter(Boolean).pop() ?? "";
        draft.name = directory ? base : base.replace(/\.(exe|lnk|url|bat|cmd)$/i, "");
      }
    }
  }

  function save() {
    if (!draft) return;
    const button = { ...draft, name: draft.name.trim(), target: draft.target.trim(), args: draft.args.trim() };
    if (!button.target) {
      error = t("custom.targetRequired");
      return;
    }
    editor.update((s) => {
      const list = s.toolbar.customButtons;
      const at = list.findIndex((b) => b.id === button.id);
      if (at >= 0) {
        list[at] = button;
      } else {
        list.push(button);
        // shown right after the start button (or first)
        const left = s.toolbar.items.left;
        const start = left.indexOf("start");
        left.splice(start + 1, 0, customItemId(button));
      }
    });
    draft = null;
  }

  function remove(button: CustomButton) {
    editor.update((s) => {
      s.toolbar.customButtons = s.toolbar.customButtons.filter((b) => b.id !== button.id);
      const id = customItemId(button);
      for (const section of ["left", "center", "right"] as const) {
        s.toolbar.items[section] = s.toolbar.items[section].filter((i) => i !== id);
      }
    });
    if (draft?.id === button.id) draft = null;
  }
</script>

{#snippet preview(button: CustomButton)}
  <span class="preview">
    {#if button.icon}
      <Icon name={button.icon} size={18} />
    {:else if isUrl(button.target)}
      <Icon name="Globe" size={18} />
    {:else if button.target}
      <img src={iconUrl(button.target)} alt="" />
    {:else}
      <Icon name="MousePointerClick" size={18} />
    {/if}
  </span>
{/snippet}

<div class="custom">
  {#if buttons.length}
    <ul class="list">
      {#each buttons as button (button.id)}
        <li>
          {@render preview(button)}
          <div class="text">
            <span class="name">{button.name || button.target}</span>
            <span class="target" title={button.target}>{button.target}{button.args ? ` ${button.args}` : ""}</span>
          </div>
          <button type="button" class="btn btn-ghost" onclick={() => startEdit(button)}>
            <Icon name="Pencil" size={14} />
            {t("custom.edit")}
          </button>
          <button type="button" class="btn btn-soft-danger" onclick={() => remove(button)}>
            <Icon name="Trash2" size={14} />
            {t("custom.remove")}
          </button>
        </li>
      {/each}
    </ul>
  {:else if !draft}
    <p class="empty">{t("custom.none")}</p>
  {/if}

  {#if draft}
    <div class="form">
      <label class="field">
        <span>{t("custom.name")}</span>
        <input type="text" maxlength="40" bind:value={draft.name} placeholder={t("custom.namePlaceholder")} />
      </label>
      <div class="field">
        <span>{t("custom.target")}</span>
        <div class="target-row">
          <input
            type="text"
            maxlength="1024"
            bind:value={draft.target}
            placeholder={t("custom.targetPlaceholder")}
            spellcheck="false"
          />
          <button type="button" class="btn" onclick={() => browse(false)}>{t("custom.pickFile")}</button>
          <button type="button" class="btn" onclick={() => browse(true)}>{t("custom.pickFolder")}</button>
        </div>
      </div>
      {#if !isUrl(draft.target)}
        <label class="field">
          <span>{t("custom.args")}</span>
          <input type="text" maxlength="1024" bind:value={draft.args} placeholder={t("custom.argsPlaceholder")} spellcheck="false" />
        </label>
      {/if}
      <div class="field">
        <span>{t("custom.icon")}</span>
        <div class="icons" role="radiogroup">
          <button
            type="button"
            class="icon-choice auto"
            class:active={!draft.icon}
            role="radio"
            aria-checked={!draft.icon}
            onclick={() => draft && (draft.icon = "")}
          >
            {@render preview({ ...draft, icon: "" })}
            <span>{t("custom.iconAuto")}</span>
          </button>
          {#each ICONS as name (name)}
            <button
              type="button"
              class="icon-choice"
              class:active={draft.icon === name}
              role="radio"
              aria-checked={draft.icon === name}
              aria-label={name}
              onclick={() => draft && (draft.icon = name)}
            >
              <Icon {name} size={17} />
            </button>
          {/each}
        </div>
      </div>
      {#if error}<p class="error">{error}</p>{/if}
      <div class="actions">
        <button type="button" class="btn btn-primary" onclick={save}>{isNew ? t("custom.add") : t("custom.save")}</button>
        <button type="button" class="btn btn-ghost" onclick={() => (draft = null)}>{t("custom.cancel")}</button>
      </div>
    </div>
  {:else if buttons.length < MAX}
    <button type="button" class="btn add" onclick={startAdd}>
      <Icon name="Plus" size={14} />
      {t("custom.new")}
    </button>
  {/if}
</div>

<style>
  .custom {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .list {
    margin: 0;
    padding: 0;
    list-style: none;

    li {
      display: flex;
      align-items: center;
      gap: 12px;
      padding: 8px 4px;
      border-bottom: 1px solid var(--border);
    }
  }

  .preview {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 32px;
    height: 32px;
    border-radius: var(--radius-s);
    background: var(--surface-2);

    img {
      width: 20px;
      height: 20px;
    }
  }

  .text {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }
  .name {
    font-weight: 600;
  }
  .target {
    overflow: hidden;
    color: var(--fg-muted);
    font-size: 12px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .empty {
    color: var(--fg-muted);
    font-size: 13px;
  }

  .form {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 14px;
    border-radius: var(--radius-m);
    border: 1px solid var(--border-strong);
    background: var(--surface-2);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 13px;

    > span {
      color: var(--fg-secondary);
      font-weight: 600;
    }
  }

  input[type="text"] {
    flex: 1;
    min-width: 0;
    height: 34px;
    padding: 0 10px;
    border-radius: var(--radius-s);
    border: 1px solid var(--border-strong);
    background: var(--surface);
    color: var(--fg);
    font: inherit;
    outline: none;

    &:focus {
      border-color: var(--accent);
    }
  }

  .target-row {
    display: flex;
    gap: 6px;
  }

  .icons {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .icon-choice {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    min-width: 36px;
    height: 36px;
    padding: 0 6px;
    border-radius: var(--radius-s);
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--fg);

    &.active {
      border-color: var(--accent);
      box-shadow: 0 0 0 1px var(--accent);
    }
    &.auto {
      padding-right: 10px;

      .preview {
        width: 24px;
        height: 24px;
        background: transparent;
      }
    }
  }

  .error {
    color: var(--danger);
    font-size: 12.5px;
  }

  .actions {
    display: flex;
    gap: 8px;
  }

  .add {
    align-self: flex-start;
  }
</style>
