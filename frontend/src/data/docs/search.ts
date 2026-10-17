import { t } from "../../i18n";
import { guides } from "./guides";
import { settings } from "./settings";

export type Route =
  | { kind: "home" }
  | { kind: "search"; query: string }
  | { kind: "guide"; id: string; anchor?: string }
  | { kind: "settings"; id: string; anchor?: string };

export type SearchEntry = {
  kind: "guide" | "page" | "setting";
  title: string;
  path: string[];
  text: string;
  route: Route;
  boost: number;
  titleKey: string;
  pathKey: string;
  textKey: string;
};

export type SearchResult = { entry: SearchEntry; score: number };

const fold = (text: string) => text.toLowerCase();

function entry(
  kind: SearchEntry["kind"],
  title: string,
  path: string[],
  text: string,
  route: Route,
  boost: number,
  extra = "",
): SearchEntry {
  return {
    kind,
    title,
    path,
    text,
    route,
    boost,
    titleKey: fold(title),
    pathKey: fold(path.join(" ")),
    textKey: fold(`${text} ${extra}`),
  };
}

function buildIndex(): SearchEntry[] {
  const entries: SearchEntry[] = [];
  for (const guide of guides) {
    entries.push(
      entry("guide", guide.title, [t("Guides")], guide.summary, { kind: "guide", id: guide.id }, 6),
    );
    let heading = "";
    guide.blocks.forEach((block, index) => {
      if (block.type === "h") {
        heading = block.text;
        return;
      }
      const text = "items" in block ? block.items.join(" ") : block.text;
      entries.push(
        entry(
          "guide",
          heading ? t("{guide}: {heading}", { guide: guide.title, heading }) : guide.title,
          [t("Guides"), guide.title],
          text,
          { kind: "guide", id: guide.id, anchor: `block-${index}` },
          0,
        ),
      );
    });
  }
  for (const page of settings.pages) {
    const footers = page.sections.flatMap((section) => (section.footer ? [section.footer] : []));
    entries.push(
      entry(
        "page",
        page.title,
        page.path.slice(0, -1),
        footers.join(" "),
        { kind: "settings", id: page.id },
        5,
      ),
    );
    page.sections.forEach((section, sectionIndex) => {
      section.items.forEach((item, itemIndex) => {
        if (item.type === "page") {
          return;
        }
        const route: Route = {
          kind: "settings",
          id: page.id,
          anchor: `item-${sectionIndex}-${itemIndex}`,
        };
        if (item.type === "note") {
          entries.push(entry("setting", page.title, page.path.slice(0, -1), item.label, route, 0));
          return;
        }
        const text = [item.description, item.options?.join(", ")].filter(Boolean).join(" ");
        const extra = [section.footer, section.header].filter(Boolean).join(" ");
        entries.push(
          entry("setting", item.label, page.path, text, route, item.description ? 3 : 2, extra),
        );
      });
    });
  }
  return entries;
}

let index: SearchEntry[] | undefined;

const unspaced = /[\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}\p{Script=Hangul}]/u;

const escapeRegExp = (text: string) => text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

export function queryWords(query: string): string[] {
  return [...new Set(fold(query).split(/\s+/).filter(Boolean))];
}

export function search(query: string, limit = 60): SearchResult[] {
  const words = queryWords(query);
  if (words.length === 0) {
    return [];
  }
  index ??= buildIndex();
  const whole = fold(query.trim());
  const starts = words.map((word) => new RegExp(`(^|[^\\p{L}\\p{N}])${escapeRegExp(word)}`, "u"));
  const results: SearchResult[] = [];
  const seen = new Set<string>();
  for (const candidate of index) {
    let score = candidate.boost;
    let matched = true;
    words.forEach((word, i) => {
      if (!matched) {
        return;
      }
      if (starts[i].test(candidate.titleKey)) {
        score += 12;
      } else if (candidate.titleKey.includes(word)) {
        score += 6;
      } else if (starts[i].test(candidate.pathKey)) {
        score += 3;
      } else if (starts[i].test(candidate.textKey)) {
        score += 1;
      } else if ((word.length > 3 || unspaced.test(word)) && candidate.textKey.includes(word)) {
        score += 0.5;
      } else {
        matched = false;
      }
    });
    if (!matched) {
      continue;
    }
    if (candidate.titleKey === whole) {
      score += 40;
    } else if (candidate.titleKey.startsWith(whole)) {
      score += 15;
    }
    const key = `${candidate.title}|${candidate.path.join(">")}|${candidate.text}`;
    if (seen.has(key)) {
      continue;
    }
    seen.add(key);
    results.push({ entry: candidate, score });
  }
  results.sort(
    (a, b) =>
      b.score - a.score ||
      a.entry.path.length - b.entry.path.length ||
      a.entry.title.length - b.entry.title.length,
  );
  return results.slice(0, limit);
}

export function snippet(text: string, words: string[], length = 180): string {
  if (text.length <= length) {
    return text;
  }
  const folded = fold(text);
  const positions = words.map((word) => folded.indexOf(word)).filter((position) => position >= 0);
  const first = positions.length > 0 ? Math.min(...positions) : 0;
  const start = Math.max(0, Math.min(first - 50, text.length - length));
  const end = Math.min(text.length, start + length);
  return `${start > 0 ? "…" : ""}${text.slice(start, end).trim()}${end < text.length ? "…" : ""}`;
}

export function highlightParts(text: string, words: string[]): { text: string; hit: boolean }[] {
  if (words.length === 0) {
    return [{ text, hit: false }];
  }
  const folded = fold(text);
  if (folded.length !== text.length) {
    return [{ text, hit: false }];
  }
  const pattern = new RegExp(
    words
      .slice()
      .sort((a, b) => b.length - a.length)
      .map(escapeRegExp)
      .join("|"),
    "g",
  );
  const parts: { text: string; hit: boolean }[] = [];
  let last = 0;
  for (const match of folded.matchAll(pattern)) {
    const start = match.index ?? 0;
    if (start > last) {
      parts.push({ text: text.slice(last, start), hit: false });
    }
    parts.push({ text: text.slice(start, start + match[0].length), hit: true });
    last = start + match[0].length;
  }
  if (last < text.length) {
    parts.push({ text: text.slice(last), hit: false });
  }
  return parts;
}

export function routeToHash(route: Route): string {
  if (route.kind === "home") {
    return "#";
  }
  if (route.kind === "search") {
    return `#search/${encodeURIComponent(route.query)}`;
  }
  const base = `#${route.kind === "guide" ? "guides" : "settings"}/${route.id}`;
  return route.anchor ? `${base}/${route.anchor}` : base;
}

export function hashToRoute(hash: string): Route {
  let decoded: string;
  try {
    decoded = decodeURIComponent(hash.replace(/^#\/?/, ""));
  } catch {
    return { kind: "home" };
  }
  if (decoded.startsWith("search/")) {
    return { kind: "search", query: decoded.slice("search/".length) };
  }
  const [kind, id, anchor] = decoded.split("/");
  if (kind === "guides" && id) {
    return { kind: "guide", id, anchor };
  }
  if (kind === "settings" && id) {
    return { kind: "settings", id, anchor };
  }
  return { kind: "home" };
}
