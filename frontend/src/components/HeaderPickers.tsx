import { createEffect, createSignal, For, type JSX, onCleanup, Show } from "solid-js";
import type { Account, Device, Moderating } from "../data/account";
import { t } from "../i18n";
import Icon from "./Icon";
import { ghostButtonClass } from "./PageParts";

export const channelKey = (channel: Moderating) => `${channel.platform}/${channel.login}`;

export const moderatedChannelKey = "moderatedChannel";

export const rememberChannel = (key: string) => {
  try {
    sessionStorage.setItem(moderatedChannelKey, key);
  } catch {}
};

type Option = {
  value: string;
  label: string;
  badge: () => JSX.Element;
  detail?: string;
};

function Picker(props: {
  label: string;
  value: string;
  options: Option[];
  fallback: Option;
  onChange: (value: string) => void;
}) {
  const [open, setOpen] = createSignal(false);
  const current = () =>
    props.options.find((option) => option.value === props.value) ?? props.fallback;
  let picker: HTMLDivElement | undefined;
  let button: HTMLButtonElement | undefined;
  let list: HTMLDivElement | undefined;

  const items = () => [...(list?.querySelectorAll<HTMLButtonElement>("[role=option]") ?? [])];
  const focusItem = (index: number) => {
    const all = items();
    all[(index + all.length) % all.length]?.focus();
  };
  const close = () => {
    setOpen(false);
    button?.focus();
  };

  createEffect(() => {
    if (!open()) {
      return;
    }
    const selected = items().findIndex((item) => item.getAttribute("aria-selected") === "true");
    focusItem(Math.max(selected, 0));
    const onPointerDown = (event: PointerEvent) => {
      if (!picker?.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("pointerdown", onPointerDown);
    onCleanup(() => document.removeEventListener("pointerdown", onPointerDown));
  });

  const onKeyDown = (event: KeyboardEvent) => {
    const index = items().indexOf(document.activeElement as HTMLButtonElement);
    if (event.key === "Escape") {
      close();
    } else if (event.key === "ArrowDown") {
      focusItem(index + 1);
    } else if (event.key === "ArrowUp") {
      focusItem(index - 1);
    } else if (event.key === "Home") {
      focusItem(0);
    } else if (event.key === "End") {
      focusItem(-1);
    } else if (event.key === "Tab") {
      setOpen(false);
      return;
    } else {
      return;
    }
    event.preventDefault();
  };

  return (
    <div ref={(element) => (picker = element)} class="min-w-0 sm:relative">
      <button
        ref={(element) => (button = element)}
        type="button"
        aria-label={props.label}
        aria-haspopup="listbox"
        aria-expanded={open()}
        title={current().label}
        onClick={() => setOpen(!open())}
        onKeyDown={(event) => {
          if (event.key === "ArrowDown" || event.key === "ArrowUp") {
            event.preventDefault();
            setOpen(true);
          }
        }}
        class={`${ghostButtonClass} h-11! w-full max-w-72 justify-start! gap-2! pr-4! pl-1.5! text-base! aria-expanded:bg-ghost-hover max-sm:w-11 max-sm:px-0! max-sm:justify-center!`}
      >
        <span class="shrink-0">{current().badge()}</span>
        <span class="min-w-0 truncate max-sm:hidden">{current().label}</span>
        <Icon
          name="chevron-down"
          class={`ml-auto size-4 shrink-0 text-ink-2 transition-transform max-sm:hidden ${open() ? "rotate-180" : ""}`}
        />
      </button>
      <Show when={open()}>
        <div
          ref={(element) => (list = element)}
          role="listbox"
          aria-label={props.label}
          onKeyDown={onKeyDown}
          class="absolute top-full z-10 mt-2 flex max-h-[min(24rem,calc(100dvh-6rem))] flex-col overflow-y-auto rounded-2xl border-[1.5px] border-line bg-card py-2 shadow-xl max-sm:inset-x-5 sm:right-0 sm:w-max sm:max-w-80 sm:min-w-64"
        >
          <For each={props.options}>
            {(option) => (
              <button
                type="button"
                role="option"
                aria-selected={option.value === props.value}
                onClick={() => {
                  close();
                  if (option.value !== props.value) {
                    props.onChange(option.value);
                  }
                }}
                class="flex w-full items-center gap-3 px-3 py-2 text-left text-ink transition-colors outline-none hover:bg-ghost focus-visible:bg-ghost aria-selected:bg-ghost"
              >
                <span class="shrink-0">{option.badge()}</span>
                <span class="flex min-w-0 flex-1 flex-col">
                  <span class="truncate font-display text-base font-semibold">{option.label}</span>
                  <Show when={option.detail}>
                    <span class="truncate text-[13px] text-muted">{option.detail}</span>
                  </Show>
                </span>
                <Icon
                  name="check"
                  class={`size-5 shrink-0 text-leaf ${option.value === props.value ? "" : "invisible"}`}
                />
              </button>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
}

export function Avatar(props: { picked: { name: string; avatar: string | null }; class?: string }) {
  return (
    <Show
      when={props.picked.avatar}
      fallback={
        <span
          class={`flex items-center justify-center rounded-full bg-moss font-display font-semibold text-white ${props.class ?? "size-8 text-sm"}`}
        >
          {props.picked.name.slice(0, 1).toUpperCase()}
        </span>
      }
    >
      {(avatar) => (
        <img
          src={avatar()}
          alt=""
          referrerpolicy="no-referrer"
          class={`rounded-full bg-ghost object-cover ${props.class ?? "size-8"}`}
        />
      )}
    </Show>
  );
}

function DeviceBadge(props: { device: Device | undefined }) {
  return (
    <span class="relative flex size-8 items-center justify-center rounded-full bg-ghost-hover text-ink">
      <Icon name="smartphone" class="size-5" />
      <span
        class={`absolute right-0 bottom-0 size-2.5 rounded-full border-2 border-card ${props.device?.connected ? "bg-leaf" : props.device?.reconnecting ? "bg-amber" : "bg-muted"}`}
      />
    </span>
  );
}

export function StreamerPicker(props: {
  account: Account;
  picked: Moderating | null;
  onPick: (key: string) => void;
}) {
  const you = (): Option => ({
    value: "",
    label: t("You"),
    badge: () => <Avatar picked={props.account} />,
  });
  return (
    <Picker
      label={t("Streamer")}
      value={props.picked ? channelKey(props.picked) : ""}
      fallback={you()}
      options={[
        you(),
        ...props.account.moderating.map((channel) => ({
          value: channelKey(channel),
          label:
            channel.platform === "kick"
              ? t("{name} on Kick", { name: channel.name })
              : channel.name,
          badge: () => <Avatar picked={channel} />,
        })),
      ]}
      onChange={props.onPick}
    />
  );
}

function DashboardBadge() {
  return (
    <span class="flex size-8 items-center justify-center rounded-full bg-ghost-hover text-ink">
      <Icon name="grid" class="size-5" />
    </span>
  );
}

export function PagePicker(props: {
  devices: Device[];
  picked: string;
  onPick: (device: string) => void;
}) {
  const dashboard = (): Option => ({
    value: "",
    label: t("Dashboard"),
    badge: () => <DashboardBadge />,
  });
  return (
    <Picker
      label={t("Page")}
      value={props.picked}
      fallback={
        props.picked
          ? {
              value: props.picked,
              label: "Moblin",
              badge: () => <DeviceBadge device={undefined} />,
            }
          : dashboard()
      }
      options={[
        dashboard(),
        ...props.devices.map((device) => ({
          value: device.id,
          label: device.name,
          detail: device.connected
            ? t("Connected")
            : device.reconnecting
              ? t("Reconnecting")
              : t("Not connected"),
          badge: () => <DeviceBadge device={device} />,
        })),
      ]}
      onChange={props.onPick}
    />
  );
}
