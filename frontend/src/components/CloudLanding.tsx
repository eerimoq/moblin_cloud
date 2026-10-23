import { createSignal, For, Show } from "solid-js";
import { asset } from "../asset";
import { cloudSteps } from "../data/cloud";
import Icon, { type IconName } from "./Icon";
import { Notice, PlatformSignIn } from "./PageParts";
import { spinsText } from "../data/remoteControl";
import { t } from "../i18n";

function SignInButtons() {
  return (
    <div class="flex flex-wrap gap-3">
      <PlatformSignIn platform="twitch" large>
        {t("Sign in with Twitch")}
      </PlatformSignIn>
      <PlatformSignIn platform="kick" large>
        {t("Sign in with Kick")}
      </PlatformSignIn>
    </div>
  );
}

function Toast(props: { icon: IconName; label: string; text: string; class: string }) {
  return (
    <div
      class={`absolute flex w-56 items-center gap-3 rounded-2xl border-[1.5px] border-line bg-card px-3.5 py-3 shadow-xl ${props.class}`}
    >
      <span class="flex size-9 shrink-0 items-center justify-center rounded-full bg-moss text-white">
        <Icon name={props.icon} class="size-[18px]" />
      </span>
      <span class="flex min-w-0 flex-col">
        <span class="text-[12px] font-semibold tracking-[0.06em] text-muted uppercase">
          {props.label}
        </span>
        <span class="truncate text-[15px] font-semibold">{props.text}</span>
      </span>
    </div>
  );
}

function PageMockup() {
  const ways: { icon: IconName; title: string }[] = [
    { icon: "heart", title: t("Just a tip") },
    { icon: "wheel", title: t("Spin the wheel") },
  ];
  return (
    <div inert aria-hidden="true" class="relative mx-auto w-full max-w-[340px] select-none">
      <div class="rounded-[28px] border-[1.5px] border-line bg-page p-3 shadow-2xl">
        <img
          src={asset("cloud-banner.svg")}
          alt=""
          class="aspect-[3/1] w-full rounded-2xl object-cover"
        />
        <div class="flex items-end gap-3 px-3">
          <img
            src={asset("cards/avatar-happy.png")}
            alt=""
            class="-mt-7 size-16 rounded-full border-4 border-page bg-ghost"
          />
          <span class="flex items-center gap-1.5 pb-1 text-[14px] text-ink-2">
            <Icon name="twitch" class="size-4 text-twitch" />
            twitch.tv/mobby
          </span>
        </div>
        <p class="px-3 pt-2 text-[14px] text-ink-2">
          {t("Thanks for hanging out! Say hi in chat.")}
        </p>
        <p class="px-3 pt-4 pb-3 font-display text-2xl font-bold">
          {t("Tip {name}", { name: "Mobby" })}
        </p>
        <div class="flex flex-col gap-2 px-1 pb-1">
          <For each={ways}>
            {(way, index) => (
              <div
                class={`flex items-center gap-3 rounded-2xl border-[1.5px] px-3 py-2.5 ${
                  index() === 0 ? "border-moss bg-well" : "border-line bg-card"
                }`}
              >
                <span
                  class={`flex size-9 items-center justify-center rounded-full ${
                    index() === 0 ? "bg-moss text-white" : "bg-well text-leaf"
                  }`}
                >
                  <Icon name={way.icon} class="size-[18px]" />
                </span>
                <span class="font-display text-[16px] font-semibold">{way.title}</span>
              </div>
            )}
          </For>
        </div>
      </div>
      <Toast
        icon="heart"
        label={t("New tip")}
        text={t("Great view up there!")}
        class="-top-6 -left-6 -rotate-3 max-sm:hidden lg:-left-20"
      />
      <Toast
        icon="wheel"
        label={t("Wheel spin")}
        text={spinsText(3)}
        class="-right-6 -bottom-8 rotate-2 max-sm:hidden lg:-right-16"
      />
    </div>
  );
}

export default function CloudLanding(props: { failed: boolean }) {
  const [dismissed, setDismissed] = createSignal(false);

  return (
    <>
      <section class="mx-auto grid max-w-[1120px] items-center gap-12 px-5 pt-10 pb-16 sm:px-8 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)] lg:gap-16 lg:pt-16 lg:pb-24">
        <div class="flex flex-col gap-6">
          <Show when={props.failed && !dismissed()}>
            <Notice status onClose={() => setDismissed(true)}>
              {t("Signing in did not work. Try again.")}
            </Notice>
          </Show>
          <span class="font-display text-[15px] font-semibold tracking-[0.08em] text-leaf uppercase">
            Moblin Cloud
          </span>
          <h1 class="text-[40px] leading-[1.05] font-bold text-balance sm:text-6xl">
            {t("Let your viewers join in.")}
          </h1>
          <p class="text-lg text-ink-2 text-pretty sm:text-[21px]">
            {t(
              "Viewers tip you from your own page, with a message or a spin of your wheel, and it " +
                "all shows up in Moblin while you stream.",
            )}
          </p>
          <SignInButtons />
        </div>
        <PageMockup />
      </section>
      <section class="bg-band py-16 lg:py-20">
        <div class="mx-auto flex max-w-[1120px] flex-col gap-10 px-5 sm:px-8">
          <div class="flex max-w-[640px] flex-col gap-2.5">
            <span class="font-display text-[15px] font-semibold tracking-[0.08em] text-leaf uppercase">
              {t("How it works")}
            </span>
            <h2 class="text-4xl font-bold sm:text-[46px]">{t("Up and running in minutes.")}</h2>
          </div>
          <ol class="grid gap-6 sm:grid-cols-3">
            <For each={cloudSteps}>
              {(step, index) => (
                <li class="flex flex-col gap-3">
                  <span class="flex size-11 items-center justify-center rounded-full bg-moss font-display text-xl font-bold text-white">
                    {index() + 1}
                  </span>
                  <h3 class="text-[23px] font-semibold">{step.title}</h3>
                  <p class="text-base text-ink-2">{step.text}</p>
                </li>
              )}
            </For>
          </ol>
          <SignInButtons />
        </div>
      </section>
    </>
  );
}
