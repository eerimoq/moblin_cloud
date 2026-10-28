import { Show, type JSX } from "solid-js";
import { asset } from "../asset";
import { links, pages } from "../data/links";
import { language, languages, setLanguage, t } from "../i18n";
import Icon, { type IconName } from "./Icon";

const footerLinks: { label: string; href: string; icon?: IconName }[] = [
  { label: "Moblin", href: links.website },
  { label: t("Docs"), href: links.docs },
  { label: t("Terms"), href: asset(pages.terms) },
  { label: "GitHub", href: links.github, icon: "github" },
  { label: "Discord", href: links.discord, icon: "discord" },
  { label: t("Privacy policy"), href: links.privacyPolicy },
];

const isExternal = (href: string) => /^https?:/.test(href);

export default function Footer(props: { note?: JSX.Element }) {
  return (
    <footer class="border-t border-line/40 py-10">
      <div class="mx-auto flex max-w-[1120px] flex-col items-center px-5 text-center sm:px-8">
        <Show when={props.note}>
          <p class="mb-6 text-[15px] text-muted">{props.note}</p>
        </Show>
        <nav class="flex flex-wrap justify-center gap-x-7 gap-y-2" aria-label="Footer">
          {footerLinks.map((l) => (
            <a
              href={l.href}
              target={isExternal(l.href) ? "_blank" : undefined}
              rel={isExternal(l.href) ? "noopener" : undefined}
              class="flex items-center gap-1.5 text-[15px] font-bold whitespace-nowrap text-muted hover:text-ink"
            >
              {l.icon && <Icon name={l.icon} class="size-4" />}
              {l.label}
            </a>
          ))}
        </nav>
        <label class="mt-6 flex items-center gap-2 text-[15px] font-bold text-muted">
          <Icon name="globe" class="size-4" />
          <span class="sr-only">{t("Language")}</span>
          <select
            class="cursor-pointer rounded-lg border-[1.5px] border-line bg-card px-2 py-1 text-ink"
            onChange={(event) => setLanguage(event.currentTarget.value)}
          >
            {languages.map((l) => (
              <option value={l.code} selected={l.code === language}>
                {l.name}
              </option>
            ))}
          </select>
        </label>
      </div>
    </footer>
  );
}
