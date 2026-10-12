import { asset } from "../asset";
import { links, pages } from "../data/links";
import Icon, { type IconName } from "./Icon";
import Mascot from "./Mascot";

const footerLinks: { label: string; href: string; icon?: IconName }[] = [
  { label: "Docs", href: asset(pages.docs) },
  { label: "Chat bot", href: asset(pages.chatBot) },
  { label: "Trading cards", href: asset(pages.cards) },
  { label: "GitHub", href: links.github, icon: "github" },
  { label: "Discord", href: links.discord, icon: "discord" },
  { label: "Privacy policy", href: links.privacyPolicy },
];

const isExternal = (href: string) => /^https?:/.test(href);

export default function Footer() {
  return (
    <footer class="border-t border-line/40 py-10">
      <div class="mx-auto flex max-w-[1120px] flex-col items-center gap-5 px-5 text-center sm:px-8 md:flex-row md:justify-between md:text-left">
        <div class="flex flex-col items-center gap-3 md:flex-row">
          <Mascot variant="happy" class="w-8" />
          <p class="text-[15px] text-muted">
            Moblin is free and open source. Made by Erik Moqvist and the Moblin community.
          </p>
        </div>
        <nav class="flex gap-7" aria-label="Footer">
          {footerLinks.map((l) => (
            <a
              href={l.href}
              target={isExternal(l.href) ? "_blank" : undefined}
              rel={isExternal(l.href) ? "noopener" : undefined}
              class="flex items-center gap-1.5 text-[15px] font-bold text-muted hover:text-ink"
            >
              {l.icon && <Icon name={l.icon} class="size-4" />}
              {l.label}
            </a>
          ))}
        </nav>
      </div>
    </footer>
  );
}
