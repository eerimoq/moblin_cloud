import { createSignal, Show, type JSX } from "solid-js";
import { asset } from "../asset";
import { pages } from "../data/links";
import CopyButton from "./CopyButton";
import Icon from "./Icon";
import { t } from "../i18n";

export const inputClass =
  "h-13 w-full rounded-2xl border-[1.5px] border-line bg-well px-4 text-[17px] text-ink placeholder:text-muted focus:border-leaf focus:outline-none";

export const buttonClass =
  "inline-flex h-13 items-center justify-center gap-2.5 rounded-full bg-moss px-6 font-display text-lg font-semibold whitespace-nowrap text-white transition-colors hover:bg-moss-bright disabled:opacity-60";

export const ghostButtonClass =
  "inline-flex h-13 items-center justify-center gap-2.5 rounded-full border-[1.5px] border-ghost-line bg-ghost px-6 font-display text-lg font-semibold whitespace-nowrap text-ink transition-colors hover:bg-ghost-hover disabled:opacity-60";

export const cardClass =
  "flex flex-col gap-5 rounded-3xl border-[1.5px] border-line bg-card p-6 sm:p-8";

export function Notice(props: { status?: boolean; error?: boolean; children: JSX.Element }) {
  return (
    <p
      role={props.error ? "alert" : props.status ? "status" : undefined}
      class={`rounded-2xl border-[1.5px] px-4 py-3 text-[15px] font-semibold ${
        props.error
          ? "border-danger-line bg-danger-bg text-danger-ink"
          : "border-amber-line bg-amber-bg text-amber-ink"
      }`}
    >
      {props.children}
    </p>
  );
}

export function Spinner() {
  return (
    <div role="status" aria-label={t("Loading")} class="flex justify-center py-12">
      <span
        class="size-10 animate-spin rounded-full border-4 border-line border-t-leaf"
        style={{ "animation-delay": `-${performance.now() % 1000}ms` }}
      />
    </div>
  );
}

export function SaveButton(props: { saving: boolean; saved: boolean; disabled: boolean }) {
  return (
    <button type="submit" class={buttonClass} disabled={props.disabled || props.saving}>
      <Show when={props.saved}>
        <Icon name="check" class="size-5" />
      </Show>
      <span aria-live="polite">
        {props.saving ? t("Saving…") : props.saved ? t("Saved") : t("Save")}
      </span>
    </button>
  );
}

export function CloudHeader(props: { children?: JSX.Element }) {
  return (
    <header class="sticky top-0 z-20 bg-page/92 backdrop-blur border-b border-line/40">
      <div class="mx-auto flex min-h-16 max-w-[1120px] flex-wrap items-center justify-between gap-x-4 gap-y-2 px-5 py-2.5 sm:px-8 lg:min-h-18">
        <a href={asset(pages.dashboard)} class="flex shrink-0 items-center gap-3">
          <img src={asset("logos/logo-default.png")} alt="" class="h-9 w-auto" />
          <span class="font-display text-2xl font-bold max-sm:sr-only">Moblin Cloud</span>
        </a>
        {props.children}
      </div>
    </header>
  );
}

export function Hero(props: { eyebrow?: string; title: string; children: JSX.Element }) {
  return (
    <section class="mx-auto flex max-w-[1120px] flex-col gap-4 px-5 pt-12 pb-10 sm:px-8 lg:pt-18">
      <Show when={props.eyebrow}>
        <span class="font-display text-[15px] font-semibold uppercase tracking-[0.08em] text-leaf">
          {props.eyebrow}
        </span>
      </Show>
      <h1 class="text-[40px] leading-[1.05] font-bold text-balance sm:text-6xl">{props.title}</h1>
      {props.children}
    </section>
  );
}

export function Band(props: { children: JSX.Element; tight?: boolean }) {
  return (
    <section class={`bg-band ${props.tight ? "py-8 lg:py-12" : "py-16 lg:py-20"}`}>
      <div class="mx-auto flex max-w-[1120px] flex-col gap-6 px-5 sm:px-8">{props.children}</div>
    </section>
  );
}

export function Value(props: {
  label: string;
  text: string;
  href?: string;
  secret?: boolean;
  hideLabel?: boolean;
}) {
  const [shown, setShown] = createSignal(false);
  const shownText = () => (props.secret && !shown() ? "•".repeat(24) : props.text);

  return (
    <div class="flex flex-col gap-2">
      <span class={props.hideLabel ? "sr-only" : "text-[15px] font-semibold text-ink-2"}>
        {props.label}
      </span>
      <span class="flex items-start gap-2 rounded-2xl border-[1.5px] border-line bg-well px-4 py-3">
        <Show
          when={props.href}
          fallback={
            <code class="min-w-0 flex-1 font-mono text-[15px] break-all">{shownText()}</code>
          }
        >
          {(href) => (
            <a
              href={href()}
              target="_blank"
              rel="noopener"
              class="min-w-0 flex-1 font-mono text-[15px] break-all text-leaf hover:text-leaf-bright"
            >
              {props.text}
            </a>
          )}
        </Show>
        <Show when={props.secret}>
          <button
            type="button"
            class="inline-flex size-7 shrink-0 items-center justify-center rounded-md text-muted transition-colors hover:bg-ghost hover:text-ink"
            aria-label={
              shown()
                ? t("Hide {label}", { label: props.label })
                : t("Show {label}", { label: props.label })
            }
            aria-pressed={shown()}
            title={shown() ? t("Hide") : t("Show")}
            onClick={() => setShown(!shown())}
          >
            <Icon name={shown() ? "eye-off" : "eye"} class="size-4" />
          </button>
        </Show>
        <CopyButton text={props.text} label={props.label} />
      </span>
    </div>
  );
}
