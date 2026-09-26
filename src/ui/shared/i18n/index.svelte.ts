import type { Language } from "../types.ts";
import { en, type TranslationKey } from "./en.ts";
import { zhCN } from "./zh-CN.ts";

const dictionaries: Record<Language, Record<TranslationKey, string>> = {
  en,
  "zh-CN": zhCN,
};

let current = $state<Language>(navigator.language.toLowerCase().startsWith("zh") ? "zh-CN" : "en");

export const i18n = {
  get language(): Language {
    return current;
  },
  set language(lang: Language) {
    current = lang in dictionaries ? lang : "en";
    document.documentElement.lang = current;
  },
};

/** Reactive translation: reading `current` makes templates re-render on language change. */
export function t(key: TranslationKey, params?: Record<string, string | number>): string {
  let text: string = dictionaries[current][key] ?? en[key] ?? key;
  if (params) {
    for (const [name, value] of Object.entries(params)) {
      text = text.replaceAll(`{${name}}`, String(value));
    }
  }
  return text;
}

export type { TranslationKey };
