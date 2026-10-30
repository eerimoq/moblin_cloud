import { createSignal, For, Index, Show, type JSX } from "solid-js";
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

export function Group(props: {
  header?: string;
  footer?: string;
  error?: string | null;
  children: JSX.Element;
}) {
  return (
    <div class="flex flex-col">
      <Show when={props.header}>
        <span class="px-4 pb-1.5 text-[13px] text-phone-border uppercase">{props.header}</span>
      </Show>
      <div class="flex flex-col divide-y divide-phone-separator overflow-hidden rounded-[10px] bg-phone-row">
        {props.children}
      </div>
      <Show when={props.footer}>
        <span class="px-4 pt-1.5 text-[13px] whitespace-pre-line text-phone-border">
          {props.footer}
        </span>
      </Show>
      <Show when={props.error}>
        <span class="px-4 pt-1.5 text-[13px] font-bold text-phone-live">{props.error}</span>
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

export function NavRow(props: {
  icon?: IconName;
  label: string;
  value?: string;
  center?: boolean;
  onClick: () => void;
}) {
  return (
    <button type="button" class={rowClass} onClick={() => props.onClick()}>
      <Show when={props.icon}>
        {(icon) => <Icon name={icon()} class="size-[18px] shrink-0 text-phone-tint" />}
      </Show>
      <span class={`min-w-0 flex-1 truncate ${props.center ? "text-center" : ""}`}>
        {props.label}
      </span>
      <Show when={props.value !== undefined}>
        <span class="max-w-[55%] shrink-0 truncate text-phone-border">{props.value}</span>
      </Show>
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
      <Switch
        label={props.label}
        on={props.on}
        disabled={props.disabled}
        onToggle={props.onToggle}
      />
    </div>
  );
}

export function TextButton(props: { label: string; disabled?: boolean; onClick: () => void }) {
  return (
    <button
      type="button"
      class={`${rowClass} justify-center text-phone-tint`}
      disabled={props.disabled}
      onClick={() => props.onClick()}
    >
      {props.label}
    </button>
  );
}

export function Switch(props: {
  label: string;
  on: boolean;
  disabled: boolean;
  onToggle: (on: boolean) => void;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={props.on}
      aria-label={props.label}
      disabled={props.disabled}
      class={`relative h-[31px] w-[51px] shrink-0 rounded-full transition-colors disabled:opacity-50 ${
        props.on ? "bg-phone-switch" : "bg-phone-separator"
      }`}
      onClick={(event) => {
        event.stopPropagation();
        props.onToggle(!props.on);
      }}
    >
      <span
        class={`absolute top-[2px] left-[2px] size-[27px] rounded-full bg-white shadow transition-transform ${
          props.on ? "translate-x-[20px]" : ""
        }`}
      />
    </button>
  );
}

export type SwipeAction = { label: string; tone: "red" | "gray"; run: () => void };

export type Handle = { start: (event: PointerEvent) => void; disabled: boolean };

export function SwitchNavRow(props: {
  label: string;
  icon?: IconName;
  on: boolean;
  disabled: boolean;
  handle?: Handle;
  onToggle: (on: boolean) => void;
  onClick: () => void;
}) {
  return (
    <div
      role="button"
      tabIndex={0}
      class={`${rowClass} cursor-default`}
      onClick={() => props.onClick()}
      onKeyDown={(event) => event.key === "Enter" && props.onClick()}
    >
      <Show when={props.handle}>
        {(handle) => (
          <span
            aria-hidden="true"
            class={`-my-2 -ml-1 grid self-stretch px-1 text-phone-border ${
              handle().disabled ? "" : "cursor-grab touch-none"
            }`}
            onPointerDown={(event) => !handle().disabled && handle().start(event)}
            onClick={(event) => event.stopPropagation()}
          >
            <Icon name="drag" class="size-[18px] self-center" />
          </span>
        )}
      </Show>
      <Show when={props.icon}>
        {(icon) => <Icon name={icon()} class="size-[18px] shrink-0 text-phone-tint" />}
      </Show>
      <span class="min-w-0 flex-1 truncate">{props.label}</span>
      <Switch
        label={props.label}
        on={props.on}
        disabled={props.disabled}
        onToggle={props.onToggle}
      />
      <Icon name="chevron-right" class="size-[16px] shrink-0 text-phone-border" />
    </div>
  );
}

const actionWidth = 80;

export function SwipeRows<T>(props: {
  items: T[];
  key: (item: T) => string;
  disabled: boolean;
  actions?: (item: T) => SwipeAction[];
  onMove?: (from: number, to: number) => void;
  children: (item: T, handle: Handle | undefined) => JSX.Element;
}) {
  const [open, setOpen] = createSignal<string | null>(null);
  const [swipe, setSwipe] = createSignal<{ key: string; offset: number } | null>(null);
  const [drag, setDrag] = createSignal<{ from: number; dy: number; height: number } | null>(null);
  const target = () => {
    const current = drag();
    return current
      ? Math.max(
          0,
          Math.min(props.items.length - 1, current.from + Math.round(current.dy / current.height)),
        )
      : -1;
  };
  const shift = (index: number) => {
    const current = drag();
    if (!current || index === current.from) {
      return current ? current.dy : 0;
    }
    const to = target();
    if (current.from < index && index <= to) {
      return -current.height;
    }
    if (to <= index && index < current.from) {
      return current.height;
    }
    return 0;
  };

  const startDrag = (index: number, event: PointerEvent) => {
    const row = (event.currentTarget as HTMLElement).closest("[data-row]") as HTMLElement | null;
    if (!row || !props.onMove) {
      return;
    }
    event.preventDefault();
    const handle = event.currentTarget as HTMLElement;
    handle.setPointerCapture(event.pointerId);
    const scale = row.getBoundingClientRect().height / row.offsetHeight || 1;
    const startY = event.clientY;
    setOpen(null);
    setDrag({ from: index, dy: 0, height: row.offsetHeight });
    const move = (moveEvent: PointerEvent) =>
      setDrag((current) => current && { ...current, dy: (moveEvent.clientY - startY) / scale });
    const stop = () => {
      handle.removeEventListener("pointermove", move);
      handle.removeEventListener("pointerup", stop);
      handle.removeEventListener("pointercancel", stop);
      const to = target();
      setDrag(null);
      if (to !== index && to >= 0) {
        props.onMove?.(index, to);
      }
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", stop);
    handle.addEventListener("pointercancel", stop);
  };

  const startSwipe = (key: string, width: number, event: PointerEvent) => {
    if (props.disabled || width === 0 || event.button !== 0) {
      return;
    }
    const row = event.currentTarget as HTMLElement;
    const scale = row.getBoundingClientRect().width / row.offsetWidth || 1;
    const startX = event.clientX;
    const startY = event.clientY;
    const base = open() === key ? -width : 0;
    let swiping = false;
    const move = (moveEvent: PointerEvent) => {
      const dx = (moveEvent.clientX - startX) / scale;
      const dy = (moveEvent.clientY - startY) / scale;
      if (!swiping && Math.abs(dx) > 8 && Math.abs(dx) > Math.abs(dy)) {
        swiping = true;
        row.setPointerCapture(moveEvent.pointerId);
      }
      if (swiping) {
        setSwipe({ key, offset: Math.max(-width, Math.min(0, base + dx)) });
      }
    };
    const stop = () => {
      row.removeEventListener("pointermove", move);
      row.removeEventListener("pointerup", stop);
      row.removeEventListener("pointercancel", stop);
      const offset = swipe()?.offset ?? base;
      setSwipe(null);
      if (swiping) {
        setOpen(offset < -width / 2 ? key : null);
        row.addEventListener("click", (click) => click.stopPropagation(), {
          capture: true,
          once: true,
        });
      } else if (open() !== null) {
        setOpen(null);
        row.addEventListener("click", (click) => click.stopPropagation(), {
          capture: true,
          once: true,
        });
      }
    };
    row.addEventListener("pointermove", move);
    row.addEventListener("pointerup", stop);
    row.addEventListener("pointercancel", stop);
  };

  return (
    <Index each={props.items}>
      {(item, index) => {
        const key = () => props.key(item());
        const actions = () => props.actions?.(item()) ?? [];
        const width = () => actions().length * actionWidth;
        const offset = () => {
          const current = swipe();
          return current?.key === key() ? current.offset : open() === key() ? -width() : 0;
        };
        return (
          <div
            data-row
            class="relative overflow-hidden bg-phone-row"
            classList={{ "z-10 shadow-lg": drag()?.from === index }}
            style={{
              transform: `translateY(${shift(index)}px)`,
              transition: drag() && drag()?.from !== index ? "transform 150ms" : undefined,
            }}
          >
            <div class="absolute inset-y-0 right-0 flex">
              <For each={actions()}>
                {(action) => (
                  <button
                    type="button"
                    class={`text-[15px] text-white ${
                      action.tone === "red" ? "bg-phone-live" : "bg-phone-border"
                    }`}
                    style={{ width: `${actionWidth}px` }}
                    onClick={() => {
                      setOpen(null);
                      action.run();
                    }}
                  >
                    {action.label}
                  </button>
                )}
              </For>
            </div>
            <div
              class="relative bg-phone-row"
              style={{
                transform: `translateX(${offset()}px)`,
                transition: swipe()?.key === key() ? undefined : "transform 200ms",
              }}
              onPointerDown={(event) => startSwipe(key(), width(), event)}
            >
              {props.children(
                item(),
                props.onMove
                  ? { start: (event) => startDrag(index, event), disabled: props.disabled }
                  : undefined,
              )}
            </div>
          </div>
        );
      }}
    </Index>
  );
}
