import { For, type JSX, Show } from "solid-js";
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

const selectClass = `${ghostButtonClass} h-11! w-full cursor-pointer appearance-none truncate pr-10! pl-12! text-base! max-sm:w-11 max-sm:px-0! max-sm:text-transparent max-sm:[&>option]:text-ink`;

function Picker(props: {
  label: string;
  badge: JSX.Element;
  value: string;
  onChange: (value: string) => void;
  children: JSX.Element;
}) {
  return (
    <div class="relative min-w-0">
      <span
        aria-hidden="true"
        class="pointer-events-none absolute top-1/2 left-1.5 -translate-y-1/2"
      >
        {props.badge}
      </span>
      <select
        aria-label={props.label}
        class={selectClass}
        value={props.value}
        onChange={(event) => props.onChange(event.currentTarget.value)}
      >
        {props.children}
      </select>
      <Icon
        name="chevron-down"
        class="pointer-events-none absolute top-1/2 right-4 size-4 -translate-y-1/2 text-ink-2 max-sm:hidden"
      />
    </div>
  );
}

function Avatar(props: { picked: { name: string; avatar: string | null } }) {
  return (
    <Show
      when={props.picked.avatar}
      fallback={
        <span class="flex size-8 items-center justify-center rounded-full bg-moss font-display text-sm font-semibold text-white">
          {props.picked.name.slice(0, 1).toUpperCase()}
        </span>
      }
    >
      {(avatar) => (
        <img
          src={avatar()}
          alt=""
          referrerpolicy="no-referrer"
          class="size-8 rounded-full bg-ghost object-cover"
        />
      )}
    </Show>
  );
}

export function StreamerPicker(props: {
  account: Account;
  picked: Moderating | null;
  onPick: (key: string) => void;
}) {
  return (
    <Picker
      label={t("Streamer")}
      badge={<Avatar picked={props.picked ?? props.account} />}
      value={props.picked ? channelKey(props.picked) : ""}
      onChange={props.onPick}
    >
      <option value="" selected={props.picked === null}>
        {t("You")}
      </option>
      <For each={props.account.moderating}>
        {(channel) => (
          <option
            value={channelKey(channel)}
            selected={props.picked !== null && channelKey(channel) === channelKey(props.picked)}
          >
            {channel.platform === "kick"
              ? t("{name} on Kick", { name: channel.name })
              : channel.name}
          </option>
        )}
      </For>
    </Picker>
  );
}

export function DevicePicker(props: {
  devices: Device[];
  picked: string;
  onPick: (device: string) => void;
}) {
  const current = () => props.devices.find((device) => device.id === props.picked);
  return (
    <Picker
      label={t("Device")}
      badge={
        <span class="relative flex size-8 items-center justify-center rounded-full bg-ghost text-ink">
          <Icon name="smartphone" class="size-5" />
          <span
            class={`absolute right-0 bottom-0 size-2.5 rounded-full border-2 border-card ${current()?.connected ? "bg-leaf" : current()?.reconnecting ? "bg-amber" : "bg-muted"}`}
          />
        </span>
      }
      value={props.picked}
      onChange={props.onPick}
    >
      <Show when={current() === undefined}>
        <option value={props.picked}>Moblin</option>
      </Show>
      <For each={props.devices}>
        {(device) => (
          <option value={device.id} selected={device.id === props.picked}>
            {device.connected || device.reconnecting
              ? device.name
              : t("{name} (not connected)", { name: device.name })}
          </option>
        )}
      </For>
    </Picker>
  );
}
