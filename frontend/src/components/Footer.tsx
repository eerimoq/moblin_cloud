import { asset } from "../asset";
import { links, pages } from "../data/links";
import Icon, { type IconName } from "./Icon";

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
      <div class="mx-auto flex max-w-[1120px] flex-col items-center px-5 text-center sm:px-8">
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
      </div>
    </footer>
  );
}
