import type { JSX } from "solid-js";
import { asset } from "../asset";
import type { SignInFailure } from "../data/account";
import { cloudName, links } from "../data/links";
import { t, tRich } from "../i18n";
import { SignInNotices } from "./PageParts";

export default function CompactLanding(props: {
  failed: SignInFailure | null;
  connecting: boolean;
  children: JSX.Element;
}) {
  return (
    <section class="mx-auto max-w-[800px] px-5 py-14 sm:px-8 lg:py-24">
      <div class="overflow-hidden rounded-3xl border-[1.5px] border-line bg-card">
        <div class="flex flex-col items-start gap-6 p-7 sm:p-12">
          <img src={asset("logos/logo-cloud.png")} alt="" class="h-16 w-auto" />
          <div class="flex max-w-full flex-col gap-2">
            <span class="font-display text-xl font-semibold [overflow-wrap:anywhere]">
              {cloudName()}
            </span>
            <span class="text-sm font-semibold text-leaf">{t("Self-hosted instance")}</span>
          </div>
          <h1 class="text-[40px] leading-[1.05] font-bold text-balance sm:text-6xl">
            {t("Connect your Moblin.")}
          </h1>
          <p class="max-w-[520px] text-lg text-ink-2 sm:text-xl">
            {t("Sign in to manage your devices and streaming settings on this instance.")}
          </p>
          <SignInNotices failed={props.failed} connecting={props.connecting} />
          {props.children}
        </div>
        <div class="border-t border-line px-7 py-5 text-center text-base text-muted sm:px-12">
          {tRich("Powered by {project}", {
            project: (
              <a
                href={links.cloudGithub}
                target="_blank"
                rel="noopener"
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
