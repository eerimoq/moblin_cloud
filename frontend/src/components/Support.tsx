import { For, Show } from "solid-js";
import { asset } from "../asset";
import { shopIcons } from "../data/features";
import { links } from "../data/links";
import { createTwitchLive } from "../twitch";
import Button from "./Button";
import Icon from "./Icon";
import Mascot from "./Mascot";
import { t } from "../i18n";

export default function Support() {
  const live = createTwitchLive();
  return (
    <section id="support" class="pt-10 pb-24">
      <div class="mx-auto flex max-w-[1120px] flex-col gap-10 px-5 sm:px-8">
        <div class="flex max-w-[640px] flex-col gap-2.5">
          <span class="font-display text-[15px] font-semibold uppercase tracking-[0.08em] text-leaf">
            {t("Support Moblin")}
          </span>
          <h2 class="text-4xl font-bold sm:text-[46px]">{t("Like Moblin? Help keep it free.")}</h2>
        </div>
        <div class="relative flex flex-col gap-5 overflow-hidden rounded-3xl border-[1.5px] border-line bg-card p-8 sm:p-12 md:flex-row md:items-center md:justify-between md:gap-10 lg:px-16">
          <Mascot
            variant="heart"
            class="pointer-events-none absolute -right-8 -bottom-10 hidden w-[130px] -rotate-[8deg] opacity-30 lg:block"
          />
          <div class="flex max-w-[560px] flex-col gap-4">
            <p class="text-lg text-ink-2">
              {t(
                "Made by Erik and the Moblin community. Sponsor the project, send a tip, or just " +
                  "drop by the stream and say hi.",
              )}
            </p>
            <div class="flex flex-col gap-4">
              <div class="flex flex-col items-start gap-4 sm:flex-row sm:items-center">
                <div class="relative shrink-0 overflow-hidden rounded-xl border-[1.5px] border-line">
                  <img
                    src={asset("screenshots/icon-shop.webp")}
                    alt={t(
                      "Top right corner of Moblin, with the app icon button next to the settings gear",
                    )}
                    class="h-36 w-auto sm:h-40"
                    width="426"
                    height="225"
                    loading="lazy"
                  />
                  <span
                    class="tap-ripple"
                    style={{
                      left: "59.3%",
                      top: "10.2%",
                      width: "11.7%",
                      height: "22.2%",
                    }}
                    aria-hidden="true"
                  />
                  <Icon
                    name="tap"
                    class="tap-hand absolute size-[24.9%] text-white drop-shadow-[0_1px_3px_rgba(0,0,0,0.95)] [stroke-width:1.9]"
                    style={{ left: "56.3%", top: "21.3%" }}
                  />
                </div>
                <p class="text-[15px] text-ink-2">
                  <strong class="font-bold text-ink">{t("Buy app icons.")}</strong>{" "}
                  {t(
                    "Tap the Moblin icon in the top right corner of the app to open the store. " +
                      "Every icon helps keep Moblin free.",
                  )}
                </p>
              </div>
              <div class="flex flex-wrap items-end gap-2.5">
                <For each={shopIcons}>
                  {(src) => <img src={asset(src)} alt="" class="h-11 w-auto" loading="lazy" />}
                </For>
              </div>
            </div>
          </div>
          <div class="flex flex-wrap gap-3 md:shrink-0 md:flex-col md:items-stretch">
            <Button href={links.paypal}>
              <Icon name="heart" class="size-[22px]" />
              {t("Tip on PayPal")}
            </Button>
            <Button href={links.sponsor} variant="ghost">
              <Icon name="github" class="size-5" />
              GitHub Sponsors
            </Button>
            <Button href={links.twitch} variant={live() ? "live" : "ghost"}>
              <Show
                when={live()}
                fallback={
                  <>
                    <Icon name="twitch" class="size-5" />
                    {t("Watch Erik on Twitch")}
                  </>
                }
              >
                <span class="relative flex size-3" aria-hidden="true">
                  <span class="absolute inline-flex size-full animate-ping rounded-full bg-live opacity-75 motion-reduce:animate-none" />
                  <span class="relative inline-flex size-3 rounded-full bg-live" />
                </span>
                {t("Erik is live on Twitch")}
              </Show>
            </Button>
          </div>
        </div>
      </div>
    </section>
  );
}
