import { createSignal, For, Show } from "solid-js";
import { asset } from "../asset";
import { links, navItems } from "../data/links";
import { t } from "../i18n";
import Button from "./Button";
import Icon from "./Icon";

export default function Nav() {
  const [open, setOpen] = createSignal(false);

  return (
    <header class="sticky top-0 z-20 bg-page/92 backdrop-blur border-b border-line/40">
      <div class="mx-auto flex h-16 max-w-[1120px] items-center justify-between px-5 sm:px-8 lg:h-18">
        <a href={asset("/#top")} class="flex items-center gap-3">
          <img src={asset("logos/logo-default.png")} alt="" class="h-9 w-auto" />
          <span class="font-display text-2xl font-bold">Moblin</span>
        </a>
        <nav class="hidden items-center gap-8 md:flex" aria-label={t("Sections")}>
          <For each={navItems}>
            {(item) => (
              <a href={asset(item.href)} class="font-bold text-ink-3 hover:text-ink">
                {item.label}
              </a>
            )}
          </For>
        </nav>
        <div class="flex items-center gap-2">
          <Button href={links.appStore} size="sm">
            {t("Download")}
          </Button>
          <button
            type="button"
            class="flex size-11 items-center justify-center rounded-full text-ink hover:bg-ghost md:hidden"
            aria-label={open() ? t("Close menu") : t("Open menu")}
            aria-expanded={open()}
            onClick={() => setOpen(!open())}
          >
            <Icon name={open() ? "close" : "menu"} class="size-6" />
          </button>
        </div>
      </div>
      <Show when={open()}>
        <nav class="border-t border-line bg-page px-5 py-3 md:hidden" aria-label={t("Sections")}>
          <For each={navItems}>
            {(item) => (
              <a
                href={asset(item.href)}
                class="block rounded-xl px-3 py-3 text-lg font-bold text-ink-3 hover:bg-card"
                onClick={() => setOpen(false)}
              >
                {item.label}
              </a>
            )}
          </For>
        </nav>
      </Show>
    </header>
  );
}
