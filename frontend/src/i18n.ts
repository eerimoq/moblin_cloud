import type { JSX } from "solid-js";
import { readStored, store } from "./storage";

export const languages = [
  { code: "en", name: "English" },
  { code: "de", name: "Deutsch" },
  { code: "es", name: "Español" },
  { code: "fr", name: "Français" },
  { code: "id", name: "Bahasa Indonesia" },
  { code: "it", name: "Italiano" },
  { code: "nl", name: "Nederlands" },
  { code: "pl", name: "Polski" },
  { code: "pt-BR", name: "Português (Brasil)" },
  { code: "pt-PT", name: "Português (Portugal)" },
  { code: "sk", name: "Slovenčina" },
  { code: "fi", name: "Suomi" },
  { code: "sv", name: "Svenska" },
  { code: "vi", name: "Tiếng Việt" },
  { code: "tr", name: "Türkçe" },
  { code: "ru", name: "Русский" },
  { code: "uk", name: "Українська" },
  { code: "hi", name: "हिन्दी" },
  { code: "ja", name: "日本語" },
  { code: "ko", name: "한국어" },
  { code: "zh-Hans", name: "简体中文" },
  { code: "zh-Hant", name: "繁體中文" },
] as const;

const catalogs = import.meta.glob<Record<string, string>>("./locales/*.json", {
  import: "default",
});

function matchLanguage(tag: string) {
  const [primary, ...rest] = tag.toLowerCase().split("-");
  if (primary === "zh") {
    return rest.some((part) => ["hant", "tw", "hk", "mo"].includes(part)) ? "zh-Hant" : "zh-Hans";
  }
  if (primary === "pt") {
    return rest.includes("pt") ? "pt-PT" : "pt-BR";
  }
  return languages.find((l) => l.code === primary)?.code;
}

function chosenLanguage() {
  const stored = readStored("language");
  if (languages.some((l) => l.code === stored)) {
    return stored!;
  }
  for (const tag of navigator.languages) {
    const code = matchLanguage(tag);
    if (code) {
      return code;
    }
  }
  return "en";
}

export function setLanguage(code: string) {
  store("language", code);
  location.reload();
}

export const language = chosenLanguage();

const strings = (await catalogs[`./locales/${language}.json`]?.()) ?? {};

export function t(english: string, values: Record<string, string | number> = {}) {
  return (strings[english] ?? english).replace(/\{(\w+)\}/g, (match, name: string) =>
    name in values ? String(values[name]) : match,
  );
}

export function fill(text: string, values: Record<string, JSX.Element>): JSX.Element {
  return text.split(/(\{\w+\})/).map((part) => {
    const name = /^\{(\w+)\}$/.exec(part)?.[1];
    return name !== undefined && name in values ? values[name] : part;
  });
}

export function tRich(english: string, values: Record<string, JSX.Element>) {
  return fill(strings[english] ?? english, values);
}

document.documentElement.lang = language;
document.title = t(document.title);
