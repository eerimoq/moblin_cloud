import { createSignal, For, Show, type JSX } from "solid-js";
import Icon, { type IconName } from "../Icon";
import { Choices, PhoneSwitch, rowClass } from "../MoblinParts";
import type { Stack } from "./Stack";
import { type RgbColor } from "../../data/moblinSettings";
import { t } from "../../i18n";

export function Section(props: { header?: string; footer?: JSX.Element; children: JSX.Element }) {
  return (
    <div class="flex flex-col">
      <Show when={props.header}>
        <span class="px-4 pb-1.5 text-[13px] text-phone-border uppercase">{props.header}</span>
      </Show>
      <div class="flex flex-col divide-y divide-phone-separator overflow-hidden rounded-[10px] bg-phone-row">
        {props.children}
      </div>
      <Show when={props.footer}>
        <span class="px-4 pt-1.5 text-[13px] text-phone-border">{props.footer}</span>
      </Show>
    </div>
  );
}

export function Footer(props: { children: JSX.Element }) {
  return <span class="px-4 text-[13px] text-phone-border">{props.children}</span>;
}

export function ErrorText(props: { text: string | null }) {
  return (
    <Show when={props.text}>
      <span class="font-bold text-phone-live">{props.text}</span>
    </Show>
  );
}

export function LinkRow(props: {
  label: string;
  value?: string;
  icon?: IconName;
  badge?: IconName;
  disabled?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      class={rowClass}
      disabled={props.disabled}
      onClick={() => props.onClick()}
    >
      <Show when={props.icon}>
        {(icon) => <Icon name={icon()} class="size-[18px] shrink-0 text-phone-tint" />}
      </Show>
      <span class="min-w-0 flex-1 truncate">{props.label}</span>
      <Show when={props.badge}>
        {(badge) => <Icon name={badge()} class="size-[18px] shrink-0 text-phone-warn" />}
      </Show>
      <Show when={props.value !== undefined}>
        <span class="max-w-[45%] truncate text-phone-border">{props.value}</span>
      </Show>
      <Icon name="chevron-right" class="size-[16px] shrink-0 text-phone-border" />
    </button>
  );
}

export function ToggleRow(props: {
  label: string;
  icon?: IconName;
  on: boolean;
  disabled?: boolean;
  onToggle: (on: boolean) => void;
}) {
  return (
    <PhoneSwitch
      label={props.label}
      icon={props.icon}
      on={props.on}
      disabled={props.disabled ?? false}
      onToggle={props.onToggle}
    />
  );
}

export function ButtonRow(props: {
  label: string;
  destructive?: boolean;
  disabled?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      class={`${rowClass} justify-center ${props.destructive ? "text-phone-live" : "text-phone-tint"}`}
      disabled={props.disabled}
      onClick={() => props.onClick()}
    >
      {props.label}
    </button>
  );
}

export function TextEditPage(props: {
  value: string;
  placeholder?: string;
  multiline?: boolean;
  footers?: string[];
  validate?: (value: string) => string | null;
  onSubmit: (value: string) => void;
  onDone?: () => void;
}) {
  const [value, setValue] = createSignal(props.value);
  const error = () => props.validate?.(value().trim()) ?? null;
  const submit = () => {
    if (error() === null && value() !== props.value) {
      props.onSubmit(value().trim());
    }
  };
  const done = () => {
    submit();
    props.onDone?.();
  };
  return (
    <Section
      footer={
        <span class="flex flex-col gap-1">
          <ErrorText text={error()} />
          <For each={props.footers ?? []}>{(footer) => <span>{footer}</span>}</For>
        </span>
      }
    >
      <Show
        when={props.multiline}
        fallback={
          <input
            type="text"
            class={`${rowClass} bg-transparent outline-none placeholder:text-phone-border`}
            value={value()}
            placeholder={props.placeholder}
            autofocus
            onInput={(event) => setValue(event.currentTarget.value)}
            onBlur={submit}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                done();
              }
            }}
          />
        }
      >
        <textarea
          class="min-h-[132px] w-full resize-none bg-transparent px-4 py-2 text-[17px] outline-none placeholder:text-phone-border"
          value={value()}
          placeholder={props.placeholder}
          autofocus
          onInput={(event) => setValue(event.currentTarget.value)}
          onBlur={submit}
        />
      </Show>
    </Section>
  );
}

export function TextRow(props: {
  stack: Stack;
  label: string;
  value: string;
  placeholder?: string;
  multiline?: boolean;
  masked?: boolean;
  footers?: string[];
  validate?: (value: string) => string | null;
  onSubmit: (value: string) => void;
}) {
  return (
    <LinkRow
      label={props.label}
      value={props.masked && props.value ? "••••••" : props.value}
      onClick={() =>
        props.stack.push({
          title: props.label,
          render: () => (
            <TextEditPage
              value={props.value}
              placeholder={props.placeholder}
              multiline={props.multiline}
              footers={props.footers}
              validate={props.validate}
              onSubmit={props.onSubmit}
              onDone={() => props.stack.pop()}
            />
          ),
        })
      }
    />
  );
}

export function NumberRow(props: {
  stack: Stack;
  label: string;
  value: number;
  minimum?: number;
  maximum?: number;
  integer?: boolean;
  footers?: string[];
  onSubmit: (value: number) => void;
}) {
  return (
    <TextRow
      stack={props.stack}
      label={props.label}
      value={String(props.value)}
      footers={props.footers}
      validate={(text) => {
        const number = Number(text.trim());
        if (text.trim() === "" || Number.isNaN(number)) {
          return t("Not a number");
        }
        if (props.integer && !Number.isInteger(number)) {
          return t("Not a number");
        }
        if (props.minimum !== undefined && number < props.minimum) {
          return t("Too small");
        }
        if (props.maximum !== undefined && number > props.maximum) {
          return t("Too big");
        }
        return null;
      }}
      onSubmit={(text) => props.onSubmit(Number(text))}
    />
  );
}

export type PickerItem = { id: string; name: string };

export function InlinePicker(props: {
  items: PickerItem[];
  selected: string | undefined;
  footers?: string[];
  onPick: (id: string) => void;
}) {
  const items = () =>
    props.selected !== undefined && !props.items.some((item) => item.id === props.selected)
      ? [...props.items, { id: props.selected, name: t("Unknown 😢") }]
      : props.items;
  return (
    <Choices
      items={items()}
      selected={props.selected}
      disabled={false}
      footer={props.footers?.join("\n")}
      onPick={props.onPick}
    />
  );
}

export function PickerRow(props: {
  stack: Stack;
  label: string;
  title?: string;
  icon?: IconName;
  items: PickerItem[];
  selected: string | undefined;
  footers?: string[];
  badge?: IconName;
  disabled?: boolean;
  onPick: (id: string) => void;
}) {
  const name = () =>
    props.items.find((item) => item.id === props.selected)?.name ??
    (props.selected === undefined ? "" : t("Unknown 😢"));
  return (
    <LinkRow
      label={props.label}
      value={name()}
      icon={props.icon}
      badge={props.badge}
      disabled={props.disabled}
      onClick={() =>
        props.stack.push({
          title: props.title ?? props.label,
          render: () => (
            <InlinePicker
              items={props.items}
              selected={props.selected}
              footers={props.footers}
              onPick={(id) => {
                props.onPick(id);
                props.stack.pop();
              }}
            />
          ),
        })
      }
    />
  );
}

export function MenuRow(props: {
  label: string;
  items: PickerItem[];
  selected: string;
  disabled?: boolean;
  onPick: (id: string) => void;
}) {
  return (
    <label class={`${rowClass} justify-between`}>
      <span class="min-w-0 flex-1 truncate">{props.label}</span>
      <select
        class="max-w-[50%] truncate bg-transparent text-right text-phone-border outline-none disabled:opacity-50"
        value={props.selected}
        disabled={props.disabled}
        onChange={(event) => props.onPick(event.currentTarget.value)}
      >
        <For each={props.items}>
          {(item) => (
            <option value={item.id} selected={item.id === props.selected}>
              {item.name}
            </option>
          )}
        </For>
      </select>
      <Icon name="chevron-down" class="size-[14px] shrink-0 text-phone-border" />
    </label>
  );
}

export function SliderRow(props: {
  label?: string;
  value: number;
  minimum: number;
  maximum: number;
  step: number;
  format?: (value: number) => string;
  onChange: (value: number) => void;
  onCommit?: (value: number) => void;
}) {
  return (
    <div class={`${rowClass} gap-3`}>
      <Show when={props.label}>
        <span class="shrink-0">{props.label}</span>
      </Show>
      <input
        type="range"
        class="min-w-0 flex-1 accent-white"
        min={props.minimum}
        max={props.maximum}
        step={props.step}
        value={props.value}
        onInput={(event) => props.onChange(Number(event.currentTarget.value))}
        onChange={(event) => props.onCommit?.(Number(event.currentTarget.value))}
      />
      <Show when={props.format}>
        {(format) => (
          <span class="w-[56px] shrink-0 text-right text-phone-border">
            {format()(props.value)}
          </span>
        )}
      </Show>
    </div>
  );
}

export function ValueEditCompact(props: {
  value: number;
  minimum: number;
  maximum: number;
  increment: number;
  mirror?: boolean;
  numericInput: boolean;
  decrementIcon: IconName;
  incrementIcon: IconName;
  onChange: (value: number) => void;
}) {
  const clamp = (value: number) => Math.min(props.maximum, Math.max(props.minimum, value));
  const add = (offset: number) => props.onChange(clamp(props.value + offset));
  const [text, setText] = createSignal<string | null>(null);
  const submitText = () => {
    const typed = text();
    if (typed === null) {
      return;
    }
    const number = Number(typed.trim());
    if (!Number.isNaN(number) && typed.trim() !== "") {
      props.onChange(clamp(number));
    }
    setText(null);
  };
  const button = (icon: IconName, offset: number, disabled: boolean) => (
    <button
      type="button"
      class="grid size-[36px] shrink-0 place-items-center text-phone-tint disabled:opacity-35"
      disabled={disabled}
      onClick={() => add(offset)}
    >
      <Icon name={icon} class="size-[28px]" />
    </button>
  );
  return (
    <div class="flex min-h-[44px] w-full items-center gap-2 px-4 py-1">
      <Show
        when={props.numericInput}
        fallback={
          <input
            type="range"
            class={`min-w-0 flex-1 accent-white ${props.mirror ? "rotate-180" : ""}`}
            min={props.minimum}
            max={props.maximum}
            step={15 * props.increment}
            value={props.value}
            onInput={(event) => props.onChange(Number(event.currentTarget.value))}
          />
        }
      >
        <input
          type="text"
          inputmode="decimal"
          class="min-w-0 flex-1 rounded-[6px] bg-phone-screen px-2 py-1 text-[17px] outline-none"
          value={text() ?? String(props.value)}
          onInput={(event) => setText(event.currentTarget.value)}
          onBlur={submitText}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              submitText();
            }
          }}
        />
      </Show>
      {button(
        props.decrementIcon,
        props.mirror ? props.increment : -props.increment,
        props.value === (props.mirror ? props.maximum : props.minimum),
      )}
      {button(
        props.incrementIcon,
        props.mirror ? -props.increment : props.increment,
        props.value === (props.mirror ? props.minimum : props.maximum),
      )}
    </div>
  );
}

const toHex = (color: RgbColor) =>
  `#${[color.red, color.green, color.blue].map((part) => Math.round(part).toString(16).padStart(2, "0")).join("")}`;

export function ColorRow(props: {
  label: string;
  color: RgbColor;
  opacity?: boolean;
  onChange: (color: RgbColor) => void;
}) {
  return (
    <div class={`${rowClass} justify-between`}>
      <span class="min-w-0 flex-1 truncate">{props.label}</span>
      <Show when={props.opacity}>
        <input
          type="range"
          class="w-[80px] accent-white"
          min={0}
          max={1}
          step={0.01}
          value={props.color.opacity ?? 1}
          title={t("Opacity")}
          onInput={(event) =>
            props.onChange({ ...props.color, opacity: Number(event.currentTarget.value) })
          }
        />
      </Show>
      <input
        type="color"
        class="size-[28px] shrink-0 cursor-pointer rounded-full border-0 bg-transparent p-0"
        value={toHex(props.color)}
        onInput={(event) => {
          const hex = event.currentTarget.value;
          props.onChange({
            ...props.color,
            red: parseInt(hex.slice(1, 3), 16),
            green: parseInt(hex.slice(3, 5), 16),
            blue: parseInt(hex.slice(5, 7), 16),
          });
        }}
      />
    </div>
  );
}

export function ReorderableList<T extends { id: string }>(props: {
  items: T[];
  render: (item: T) => JSX.Element;
  onMove?: (from: number, to: number) => void;
  onDelete?: (item: T) => void;
  deleteDisabled?: (item: T) => boolean;
}) {
  const [dragging, setDragging] = createSignal<number | null>(null);
  return (
    <For each={props.items}>
      {(item, index) => (
        <div
          class={`flex min-h-[44px] items-center ${dragging() === index() ? "opacity-50" : ""}`}
          draggable={props.onMove !== undefined}
          onDragStart={(event) => {
            setDragging(index());
            event.dataTransfer?.setData("text/plain", String(index()));
          }}
          onDragOver={(event) => event.preventDefault()}
          onDrop={(event) => {
            event.preventDefault();
            const from = dragging();
            setDragging(null);
            if (from !== null && from !== index()) {
              props.onMove?.(from, index());
            }
          }}
          onDragEnd={() => setDragging(null)}
        >
          <Show when={props.onMove}>
            <span class="cursor-grab pl-4 text-phone-border">
              <Icon name="menu" class="size-[18px]" />
            </span>
          </Show>
          <div class="min-w-0 flex-1">{props.render(item)}</div>
          <Show when={props.onDelete}>
            {(onDelete) => (
              <button
                type="button"
                aria-label={t("Remove")}
                class="grid size-[44px] shrink-0 place-items-center text-phone-live disabled:opacity-35"
                disabled={props.deleteDisabled?.(item)}
                onClick={() => onDelete()(item)}
              >
                <Icon name="trash" class="size-[18px]" />
              </button>
            )}
          </Show>
        </div>
      )}
    </For>
  );
}

export function PlainRow(props: { children: JSX.Element }) {
  return <div class={rowClass}>{props.children}</div>;
}

export function ValueRow(props: { label: string; value: string }) {
  return (
    <div class={rowClass}>
      <span class="min-w-0 flex-1 truncate">{props.label}</span>
      <span class="max-w-[55%] truncate text-phone-border">{props.value}</span>
    </div>
  );
}
