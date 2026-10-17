import { For, Show } from "solid-js";
import { asset } from "../asset";
import { features } from "../data/features";
import Icon from "./Icon";
import { t } from "../i18n";

export default function Features() {
  return (
    <section id="features" class="pt-10 pb-24">
      <div class="mx-auto flex max-w-[1120px] flex-col gap-10 px-5 sm:px-8">
        <div class="flex max-w-[640px] flex-col gap-2.5">
          <span class="font-display text-[15px] font-semibold uppercase tracking-[0.08em] text-leaf">
            {t("Features")}
          </span>
          <h2 class="text-4xl font-bold sm:text-[46px]">
            {t("Everything a walk-and-talk stream needs.")}
          </h2>
        </div>
        <ul class="grid gap-4 sm:grid-cols-2 sm:gap-6 lg:grid-cols-3">
          <For each={features}>
            {(f) => (
              <li class="relative flex flex-col gap-3.5 overflow-hidden rounded-3xl border-[1.5px] border-line bg-card p-7">
                {/* Sits behind the card text, so the layout matches every other card. */}
                <Show when={f.shots}>
                  {(shots) => (
                    <div class="pointer-events-none absolute -top-4 -right-3 flex gap-2 opacity-25 dark:opacity-40">
                      <For each={shots()}>
                        {(shot, i) => (
                          <img
                            src={asset(shot.src)}
                            alt={shot.alt}
                            class={`w-[82px] ${i() === 0 ? "-rotate-[6deg]" : "rotate-[6deg]"}`}
                            width="400"
                            height="650"
                            loading="lazy"
                          />
                        )}
                      </For>
                    </div>
                  )}
                </Show>
                <Icon name={f.icon} class="relative size-7 text-leaf" />
                <h3 class="relative text-[23px] font-semibold">{f.title}</h3>
                <p class="relative text-base text-ink-2">{f.text}</p>
              </li>
            )}
          </For>
        </ul>
      </div>
    </section>
  );
}
