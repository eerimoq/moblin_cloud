import { Show, type JSX } from "solid-js";
import { asset } from "../asset";
import { cloudName } from "../data/links";
import { t, tRich } from "../i18n";
import { Notice } from "./PageParts";

export default function SelfHostedLanding(props: {
  failed: boolean;
  connecting: boolean;
  children: JSX.Element;
}) {
  return (
    <section class="mx-auto max-w-[800px] px-5 py-14 sm:px-8 lg:py-24">
      <div class="overflow-hidden rounded-3xl border-[1.5px] border-line bg-card">
        <div class="flex flex-col items-start gap-6 p-7 sm:p-12">
          <img src={asset("logos/logo-cloud.png")} alt="" class="h-16 w-auto" />
          <div class="flex max-w-full flex-col gap-2">
            <span class="font-display text-xl font-semibold break-words">{cloudName()}</span>
            <span class="text-sm font-semibold text-leaf">{t("Self-hosted instance")}</span>
          </div>
          <h1 class="text-[40px] leading-[1.05] font-bold text-balance sm:text-6xl">
            {t("Connect your Moblin.")}
          </h1>
          <p class="max-w-[520px] text-lg text-ink-2 sm:text-xl">
            {t("Sign in to manage your devices and streaming settings on this instance.")}
          </p>
          <Show when={props.failed}>
            <Notice status>{t("Signing in did not work. Try again.")}</Notice>
          </Show>
          <Show when={props.connecting}>
            <Notice status>{t("Sign in to let mobtop use your account.")}</Notice>
          </Show>
          {props.children}
        </div>
        <div class="border-t border-line px-7 py-5 text-center text-base text-muted sm:px-12">
          {tRich("Powered by {project}", {
            project: (
              <a
                href="https://github.com/eerimoq/moblin_cloud"
                class="font-semibold text-leaf hover:text-leaf-bright"
              >
                Moblin Cloud
              </a>
            ),
          })}
        </div>
      </div>
    </section>
  );
}
