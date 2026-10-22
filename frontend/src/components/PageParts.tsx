import { createEffect, createSignal, For, onCleanup, Show, type JSX } from "solid-js";
import { asset } from "../asset";
import { signInUrl } from "../data/account";
import { pages } from "../data/links";
import { type Platform, platformNames } from "../data/remoteControl";
import CopyButton from "./CopyButton";
import Icon, { type IconName } from "./Icon";
import { t } from "../i18n";

export const inputClass =
  "h-13 w-full rounded-2xl border-[1.5px] border-line bg-well px-4 text-[17px] text-ink placeholder:text-muted focus:border-leaf focus:outline-none";

export const buttonClass =
  "inline-flex h-13 items-center justify-center gap-2.5 rounded-full bg-moss px-6 font-display text-lg font-semibold whitespace-nowrap text-white transition-colors hover:bg-moss-bright disabled:opacity-60";

export const ghostButtonClass =
  "inline-flex h-13 items-center justify-center gap-2.5 rounded-full border-[1.5px] border-ghost-line bg-ghost px-6 font-display text-lg font-semibold whitespace-nowrap text-ink transition-colors hover:bg-ghost-hover disabled:opacity-60";

export const cardClass =
  "flex flex-col gap-5 rounded-3xl border-[1.5px] border-line bg-card p-6 sm:p-8";

export function CloseNotice(props: { onClose: () => void }) {
  return (
    <button
      type="button"
      aria-label={t("Close")}
      onClick={() => props.onClose()}
      class="-my-1 -mr-2 ml-auto grid size-8 shrink-0 place-items-center rounded-full opacity-70 hover:bg-black/10 hover:opacity-100"
    >
      <Icon name="close" class="size-4" />
    </button>
  );
}

export function Notice(props: {
  status?: boolean;
  error?: boolean;
  onClose?: () => void;
  children: JSX.Element;
}) {
  return (
    <p
      role={props.error ? "alert" : props.status ? "status" : undefined}
      class={`flex items-start gap-3 rounded-2xl border-[1.5px] px-4 py-3 text-[15px] font-semibold ${
        props.error
          ? "border-danger-line bg-danger-bg text-danger-ink"
          : "border-amber-line bg-amber-bg text-amber-ink"
      }`}
    >
      <span>{props.children}</span>
      <Show when={props.onClose}>{(onClose) => <CloseNotice onClose={onClose()} />}</Show>
    </p>
  );
}

export const platformButtonClass = (platform: Platform) =>
  platform === "twitch"
    ? "bg-twitch text-white hover:bg-twitch-bright"
    : "bg-kick text-black hover:bg-kick-bright";

export function PlatformSignIn(props: {
  platform: Platform;
  href?: string;
  large?: boolean;
  children?: JSX.Element;
}) {
  return (
    <a
      href={props.href ?? signInUrl(props.platform)}
      aria-label={t("Sign in with {platform}", { platform: platformNames[props.platform] })}
      class={`inline-flex items-center justify-center rounded-full font-display font-semibold whitespace-nowrap transition-colors ${
        props.large ? "h-13 gap-2.5 px-6 text-lg" : "h-11 gap-2 px-5 text-base"
      } ${platformButtonClass(props.platform)}`}
    >
      <Icon name={props.platform} class="size-5" />
      {props.children ?? platformNames[props.platform]}
    </a>
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

export function CloudHeader(props: { premium?: boolean; children?: JSX.Element }) {
  return (
    <header class="sticky top-0 z-20 bg-page/92 backdrop-blur border-b border-line/40">
      <div class="mx-auto flex min-h-16 max-w-[1120px] flex-wrap items-center justify-between gap-x-4 gap-y-2 px-5 py-2.5 sm:px-8 lg:min-h-18">
        <a
          href={asset(pages.dashboard)}
          class="flex shrink-0 items-center gap-3"
          aria-label="Moblin Cloud"
        >
          <img src={asset("logos/logo-cloud.png")} alt="" class="h-9 w-auto" />
          <span class="flex flex-col">
            <span class="font-display text-2xl leading-tight font-bold" aria-hidden="true">
              <span class="max-sm:hidden">Moblin Cloud</span>
              <span class="sm:hidden">Cloud</span>
            </span>
            <Show when={props.premium}>
              <span class="-mt-1 font-display text-[11px] leading-none font-bold tracking-[0.12em] text-leaf uppercase">
                {t("Premium")}
              </span>
            </Show>
          </span>
        </a>
        {props.children}
      </div>
    </header>
  );
}

const menuItemClass =
  "flex w-full items-center gap-3 px-4 py-2.5 text-left text-[17px] text-ink transition-colors hover:bg-ghost";

export function AccountMenu(props: {
  name: string;
  avatar: string | null;
  items?: ({ label: string; icon: IconName } & ({ onClick: () => void } | { href: string }))[];
  onSignOut: () => void | Promise<void>;
}) {
  const [open, setOpen] = createSignal(false);
  let menu: HTMLDivElement | undefined;

  createEffect(() => {
    if (!open()) {
      return;
    }
    const onPointerDown = (event: PointerEvent) => {
      if (!menu?.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setOpen(false);
      }
    };
    document.addEventListener("pointerdown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    onCleanup(() => {
      document.removeEventListener("pointerdown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
    });
  });

  return (
    <div ref={(element) => (menu = element)} class="relative ml-auto shrink-0">
      <button
        type="button"
        aria-label={t("Account")}
        aria-haspopup="menu"
        aria-expanded={open()}
        class="flex size-11 items-center justify-center rounded-full transition-shadow hover:ring-2 hover:ring-ghost-line aria-expanded:ring-2 aria-expanded:ring-ghost-line"
        onClick={() => setOpen(!open())}
      >
        <Show
          when={props.avatar}
          fallback={
            <span class="flex size-11 items-center justify-center rounded-full bg-moss font-display text-lg font-semibold text-white">
              {props.name.slice(0, 1).toUpperCase()}
            </span>
          }
        >
          {(avatar) => (
            <img
              src={avatar()}
              alt=""
              referrerpolicy="no-referrer"
              class="size-11 rounded-full bg-ghost object-cover"
            />
          )}
        </Show>
      </button>
      <Show when={open()}>
        <div
          role="menu"
          class="absolute top-full right-0 z-10 mt-2 w-max min-w-48 max-w-[calc(100vw-2.5rem)] overflow-hidden rounded-2xl border-[1.5px] border-line bg-card py-2 shadow-xl"
        >
          <p class="truncate border-b border-line px-4 pt-1.5 pb-3 font-display text-base font-semibold">
            {props.name}
          </p>
          <For each={props.items ?? []}>
            {(item) =>
              "href" in item ? (
                <a href={item.href} role="menuitem" class={menuItemClass}>
                  <Icon name={item.icon} class="size-5 text-ink-2" />
                  {item.label}
                </a>
              ) : (
                <button
                  type="button"
                  role="menuitem"
                  class={menuItemClass}
                  onClick={() => {
                    setOpen(false);
                    item.onClick();
                  }}
                >
                  <Icon name={item.icon} class="size-5 text-ink-2" />
                  {item.label}
                </button>
              )
            }
          </For>
          <button
            type="button"
            role="menuitem"
            class={menuItemClass}
            onClick={() => {
              setOpen(false);
              void props.onSignOut();
            }}
          >
            <Icon name="sign-out" class="size-5 text-ink-2" />
            {t("Sign out")}
          </button>
        </div>
      </Show>
    </div>
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

export function Value(props: { label: string; text: string; href?: string; secret?: boolean }) {
  const [shown, setShown] = createSignal(false);
  const shownText = () => (props.secret && !shown() ? "•".repeat(24) : props.text);

  return (
    <div class="flex flex-col gap-2">
      <span class="text-[15px] font-semibold text-ink-2">{props.label}</span>
      <span class="flex items-start gap-2 rounded-2xl border-[1.5px] border-line bg-well px-4 py-3">
        <Show
          when={props.href}
          fallback={
            <code class="min-w-0 flex-1 font-mono text-[15px] leading-7 break-all">
              {shownText()}
            </code>
          }
        >
          {(href) => (
            <a
              href={href()}
              target="_blank"
              rel="noopener"
              class="min-w-0 flex-1 font-mono text-[15px] leading-7 break-all text-leaf hover:text-leaf-bright"
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
