import { For } from "solid-js";
import { asset } from "../asset";
import { links, platforms } from "../data/links";
import Button from "./Button";
import BrandIcon from "./BrandIcon";
import Icon from "./Icon";
import Mascot from "./Mascot";
import { t, tRich } from "../i18n";

type Confetti = {
  /** Shape and color classes. */
  shape: string;
  left: string;
  /** Seconds for one trip down the hero. */
  fall: number;
  /** Negative delay so the piece is already mid-fall on load. */
  delay: number;
  sway: number;
  /** Resting position when the viewer prefers reduced motion. */
  rest: string;
};

const confetti: Confetti[] = [
  {
    shape: "size-[18px] bg-amber",
    left: "53%",
    fall: 13,
    delay: -2,
    sway: 2.6,
    rest: "8%",
  },
  {
    shape: "size-[14px] rounded-full bg-confetti-blue",
    left: "78%",
    fall: 11,
    delay: -7,
    sway: 3.4,
    rest: "4%",
  },
  {
    shape: "h-[26px] w-[12px] bg-confetti-red",
    left: "20%",
    fall: 15,
    delay: -11,
    sway: 2.2,
    rest: "18%",
  },
  {
    shape: "size-4 bg-confetti-purple",
    left: "94%",
    fall: 12,
    delay: -4,
    sway: 3.8,
    rest: "30%",
  },
  {
    shape: "size-[14px] rounded-full bg-amber",
    left: "37%",
    fall: 14,
    delay: -9,
    sway: 3,
    rest: "62%",
  },
  {
    shape: "h-6 w-3 bg-confetti-blue",
    left: "86%",
    fall: 10,
    delay: -1,
    sway: 2.8,
    rest: "70%",
  },
  {
    shape: "size-3 rounded-full bg-confetti-red",
    left: "6%",
    fall: 12,
    delay: -6,
    sway: 3.2,
    rest: "44%",
  },
  {
    shape: "h-5 w-[10px] bg-confetti-purple",
    left: "65%",
    fall: 16,
    delay: -13,
    sway: 2.4,
    rest: "54%",
  },
  {
    shape: "size-[13px] bg-amber",
    left: "100%",
    fall: 11,
    delay: -3,
    sway: 3.6,
    rest: "84%",
  },
  {
    shape: "h-[22px] w-[11px] bg-confetti-blue",
    left: "29%",
    fall: 14,
    delay: -10,
    sway: 2.9,
    rest: "90%",
  },
  {
    shape: "size-[15px] bg-confetti-purple",
    left: "12%",
    fall: 13,
    delay: -8,
    sway: 3.1,
    rest: "12%",
  },
  {
    shape: "h-5 w-[9px] bg-amber",
    left: "45%",
    fall: 15,
    delay: -5,
    sway: 2.5,
    rest: "36%",
  },
  {
    shape: "size-[12px] rounded-full bg-confetti-blue",
    left: "71%",
    fall: 11,
    delay: -12,
    sway: 3.3,
    rest: "50%",
  },
  {
    shape: "h-[24px] w-[12px] bg-confetti-red",
    left: "0%",
    fall: 16,
    delay: -2,
    sway: 2.7,
    rest: "76%",
  },
  {
    shape: "size-[13px] bg-amber",
    left: "59%",
    fall: 12,
    delay: -14,
    sway: 3.5,
    rest: "22%",
  },
  {
    shape: "size-3 rounded-full bg-confetti-purple",
    left: "90%",
    fall: 14,
    delay: -7,
    sway: 2.3,
    rest: "66%",
  },
];

export default function Hero() {
  return (
    <section id="top" class="overflow-hidden">
      <div class="mx-auto grid max-w-[1120px] items-center gap-12 px-5 pt-12 pb-16 sm:px-8 lg:grid-cols-2 lg:pt-18 lg:pb-22">
        <div class="flex flex-col gap-6">
          <span class="font-display text-sm font-semibold uppercase tracking-[0.08em] text-leaf sm:text-[15px]">
            {t("Free · Open source · iPhone, iPad and Mac")}
          </span>
          <h1 class="text-[46px] leading-[1.02] font-bold text-balance sm:text-6xl lg:text-[76px] lg:leading-none">
            {t("Stream IRL straight from your iPhone.")}
          </h1>
          <p class="max-w-[520px] text-lg leading-normal text-ink-2 text-pretty sm:text-[21px]">
            {t(
              "Moblin is a free app for going live on Twitch, YouTube, Kick and more. Chat on your " +
                "screen, bonding for bad signal, and a friendly green face to keep you company.",
            )}
          </p>
          <div class="mt-1 flex flex-col items-start gap-4 sm:flex-row sm:items-center">
            <a href={links.appStore} target="_blank" rel="noopener" class="shrink-0">
              <img
                src={asset("badges/app-store.svg")}
                alt={t("Download on the App Store")}
                class="h-14 w-auto"
              />
            </a>
            <Button href={links.discord} variant="ghost" class="h-14">
              <Icon name="discord" class="size-[22px]" />
              {t("Join our Discord")}
            </Button>
          </div>
          <p class="text-[15px] text-muted">
            {tRich("Want the newest features first? {link}.", {
              link: (
                <a
                  href={links.testFlight}
                  target="_blank"
                  rel="noopener"
                  class="text-leaf hover:text-leaf-bright"
                >
                  {t("Join the TestFlight beta")}
                </a>
              ),
            })}
          </p>
        </div>
        <div class="relative mx-auto w-full max-w-[600px] lg:flex lg:flex-col lg:justify-center lg:self-stretch">
          <div
            class="pointer-events-none absolute inset-0 overflow-hidden lg:-top-18"
            aria-hidden="true"
          >
            <For each={confetti}>
              {(c) => (
                <span
                  class={`confetti ${c.shape}`}
                  style={{
                    left: c.left,
                    "--fall": `${c.fall}s`,
                    "--delay": `${c.delay}s`,
                    "--sway": `${c.sway}s`,
                    "--rest": c.rest,
                  }}
                />
              )}
            </For>
          </div>
          <div class="relative pt-6 pb-12 pl-10 sm:pl-16 lg:pl-20">
            <img
              src={asset("screenshots/moblin.webp")}
              srcset={`${asset("screenshots/moblin-800.webp")} 800w, ${asset("screenshots/moblin.webp")} 1146w`}
              sizes="(min-width: 1024px) 500px, 90vw"
              alt={t("Moblin streaming with chat, bitrate and controls on screen")}
              class="w-full -rotate-[4deg] drop-shadow-[0_30px_60px_rgba(0,0,0,0.55)]"
              width="1146"
              height="557"
              fetchpriority="high"
            />
            <Mascot
              variant="party"
              sticker
              class="absolute -bottom-1 -left-1 w-[120px] rotate-[8deg] sm:w-[160px] lg:w-[190px]"
            />
          </div>
        </div>
      </div>
      <div class="mx-auto flex max-w-[1120px] flex-wrap items-center gap-3 px-5 pb-16 sm:px-8 lg:pb-18">
        <span class="mr-1 text-[15px] font-bold text-muted">{t("Goes live on")}</span>
        <For each={platforms}>
          {(p) => (
            <span class="inline-flex h-10 items-center gap-2 rounded-full border-[1.5px] border-line bg-card pr-4 pl-2.5 text-[15px] font-bold text-ink-3">
              <BrandIcon name={p.icon} class="size-5" />
              {p.name}
            </span>
          )}
        </For>
        <span class="ml-1 text-[15px] text-muted">
          {t("and anything that speaks RTMP or SRT.")}
        </span>
      </div>
    </section>
  );
}
