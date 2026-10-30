import { For, Show, type JSX } from "solid-js";
import Icon, { type IconName } from "./Icon";
import { type Control, type DeviceSettings, type DeviceState, type Named } from "../data/account";
import { t } from "../i18n";

export const panelWidth = 350;

export type Remote = {
  state: DeviceState;
  settings: DeviceSettings | null;
  disabled: (key: string) => boolean;
  control: (control: Control) => Promise<void>;
};

export const rowClass =
  "flex min-h-[44px] w-full items-center gap-3 px-4 py-2 text-left text-[17px] disabled:opacity-50";

export function PanelButtons(props: {
  hidden: boolean;
  onToggleHidden: () => void;
  onClose: () => void;
}) {
  const buttonClass =
    "grid size-[30px] place-items-center rounded-full border border-phone-border text-phone-border";
  return (
    <div
      class={`absolute top-[3px] z-20 flex gap-[14px] rounded-[7px] p-[4px] ${
        props.hidden ? "right-[2px] bg-phone-row" : "right-[3px]"
      }`}
    >
      <button
        type="button"
        aria-label={props.hidden ? t("Show panel") : t("Hide panel")}
        title={props.hidden ? t("Show panel") : t("Hide panel")}
        class={buttonClass}
        onClick={() => props.onToggleHidden()}
      >
        <Icon name={props.hidden ? "eye" : "eye-off"} class="size-[18px]" />
      </button>
      <button
        type="button"
        aria-label={t("Close")}
        class={buttonClass}
        onClick={() => props.onClose()}
      >
        <Icon name="close" class="size-[14px]" />
      </button>
    </div>
  );
}

export function PanelView(props: {
  title: string;
  back?: { label: string; onBack: () => void };
  children: JSX.Element;
}) {
  return (
    <div class="flex h-full shrink-0 flex-col bg-phone-screen" style={{ width: `${panelWidth}px` }}>
      <div class="relative flex h-[50px] shrink-0 items-center justify-center px-[100px]">
        <Show when={props.back}>
          {(back) => (
            <button
              type="button"
              class="absolute left-[8px] flex max-w-[100px] items-center gap-0.5 text-[17px] text-phone-tint"
              onClick={() => back().onBack()}
            >
              <Icon name="chevron-left" class="size-[20px] shrink-0" />
              <span class="truncate">{back().label}</span>
            </button>
          )}
        </Show>
        <span class="truncate text-[17px] font-semibold">{props.title}</span>
      </div>
      <div class="flex min-h-0 flex-1 flex-col gap-6 overflow-y-auto px-4 pb-6 [scrollbar-width:none]">
        {props.children}
      </div>
    </div>
  );
}

export function Group(props: { footer?: string; children: JSX.Element }) {
  return (
    <div class="flex flex-col">
      <div class="flex flex-col divide-y divide-phone-separator overflow-hidden rounded-[10px] bg-phone-row">
        {props.children}
      </div>
      <Show when={props.footer}>
        <span class="px-4 pt-1.5 text-[13px] text-phone-border">{props.footer}</span>
      </Show>
    </div>
  );
}

export function Empty(props: { text: string }) {
  return <p class="pt-2 text-center text-[15px] text-phone-border">{props.text}</p>;
}

export function Loading() {
  return <Empty text={t("Loading…")} />;
}

export function NavRow(props: { icon?: IconName; label: string; onClick: () => void }) {
  return (
    <button type="button" class={rowClass} onClick={() => props.onClick()}>
      <Show when={props.icon}>
        {(icon) => <Icon name={icon()} class="size-[18px] shrink-0 text-phone-tint" />}
      </Show>
      <span class="min-w-0 flex-1 truncate">{props.label}</span>
      <Icon name="chevron-right" class="size-[16px] shrink-0 text-phone-border" />
    </button>
  );
}

export function Choices(props: {
  items: Named[];
  selected: string | undefined;
  icon?: IconName;
  disabled: boolean;
  footer?: string;
  onPick: (id: string) => void;
}) {
  return (
    <Show when={props.items.length > 0} fallback={<Empty text={t("Nothing here.")} />}>
      <Group footer={props.footer}>
        <For each={props.items}>
          {(item) => (
            <button
              type="button"
              class={rowClass}
              disabled={props.disabled}
              onClick={() => item.id !== props.selected && props.onPick(item.id)}
            >
              <Show when={props.icon}>
                {(icon) => <Icon name={icon()} class="size-[18px] shrink-0 text-phone-tint" />}
              </Show>
              <span class="min-w-0 flex-1 truncate">{item.name}</span>
              <Show when={item.id === props.selected}>
                <Icon name="check" class="size-[18px] text-phone-tint" />
              </Show>
            </button>
          )}
        </For>
      </Group>
    </Show>
  );
}

export function PhoneSwitch(props: {
  label: string;
  icon?: IconName;
  on: boolean;
  disabled: boolean;
  onToggle: (on: boolean) => void;
}) {
  return (
    <div class={`${rowClass} justify-between`}>
      <Show when={props.icon}>
        {(icon) => <Icon name={icon()} class="size-[18px] shrink-0 text-phone-tint" />}
      </Show>
      <span class="min-w-0 flex-1 truncate">{props.label}</span>
      <button
        type="button"
        role="switch"
        aria-checked={props.on}
        aria-label={props.label}
        disabled={props.disabled}
        class={`relative h-[31px] w-[51px] shrink-0 rounded-full transition-colors disabled:opacity-50 ${
          props.on ? "bg-phone-switch" : "bg-phone-separator"
        }`}
        onClick={() => props.onToggle(!props.on)}
      >
        <span
          class={`absolute top-[2px] left-[2px] size-[27px] rounded-full bg-white shadow transition-transform ${
            props.on ? "translate-x-[20px]" : ""
          }`}
        />
      </button>
    </div>
  );
}
