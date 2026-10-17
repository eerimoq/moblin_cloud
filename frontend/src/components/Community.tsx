import { links } from "../data/links";
import Button from "./Button";
import Icon from "./Icon";
import Mascot from "./Mascot";
import { t } from "../i18n";

export default function Community() {
  return (
    <section id="community" class="bg-band pt-10 pb-24">
      <div class="mx-auto flex max-w-[1120px] flex-col gap-10 px-5 sm:px-8">
        <div class="flex max-w-[640px] flex-col gap-2.5">
          <span class="font-display text-[15px] font-semibold uppercase tracking-[0.08em] text-leaf">
            {t("Community")}
          </span>
          <h2 class="text-4xl font-bold sm:text-[46px]">
            {t("Stuck? Someone on Discord has been there.")}
          </h2>
        </div>
        <div class="relative grid items-start gap-10 overflow-hidden rounded-3xl border-[1.5px] border-line bg-card p-8 sm:p-12 md:grid-cols-2 lg:px-16">
          <div class="flex flex-col gap-5">
            <p class="max-w-[480px] text-lg text-ink-2">
              {t(
                "Thousands of IRL streamers share setups, fix problems and vote on what Moblin does " +
                  "next. The developer hangs out there too.",
              )}
            </p>
            <div class="flex flex-wrap gap-3">
              <Button href={links.discord}>
                <Icon name="discord" class="size-[22px]" />
                {t("Join our Discord")}
              </Button>
              <Button href={links.github} variant="ghost">
                <Icon name="github" class="size-[22px]" />
                {t("Star on GitHub")}
              </Button>
              <Button href={links.testFlight} variant="ghost">
                {t("Try the beta")}
              </Button>
            </div>
          </div>
          <div class="relative hidden h-[260px] items-center justify-center md:flex">
            <Mascot variant="discord" class="w-[220px] -rotate-[6deg]" />
            <Mascot
              variant="heart"
              class="absolute right-10 -bottom-2.5 w-[120px] rotate-[10deg]"
            />
          </div>
        </div>
      </div>
    </section>
  );
}
