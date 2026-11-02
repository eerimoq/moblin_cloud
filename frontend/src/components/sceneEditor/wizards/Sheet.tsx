import { Show } from "solid-js";
import Icon, { type IconName } from "../../Icon";
import { createStack, NavigationView, type Page, type Stack } from "../Stack";
import { t } from "../../../i18n";

export function Sheet(props: { root: (stack: Stack) => Page; onClose: () => void }) {
  const stack = createStack();
  const root = props.root(stack);
  return (
    <div
      class="absolute inset-0 z-40 grid place-items-center bg-black/60"
      onPointerDown={(event) => event.stopPropagation()}
    >
      <div class="relative h-[360px] w-[560px] overflow-hidden rounded-[14px] bg-phone-screen shadow-2xl">
        <NavigationView
          stack={stack}
          root={root}
          trailing={
            <button
              type="button"
              aria-label={t("Close")}
              class="grid size-[32px] place-items-center text-phone-tint"
              onClick={() => props.onClose()}
            >
              <Icon name="close" class="size-[18px]" />
            </button>
          }
        />
      </div>
    </div>
  );
}

export function NextRow(props: { disabled?: boolean; onClick: () => void }) {
  return (
    <div class="flex flex-col overflow-hidden rounded-[10px] bg-phone-row">
      <button
        type="button"
        class="flex min-h-[44px] w-full items-center justify-center px-4 py-2 text-[17px] text-phone-tint disabled:opacity-35"
        disabled={props.disabled}
        onClick={() => props.onClick()}
      >
        {t("Next")}
      </button>
    </div>
  );
}

export function CheckList<T extends { id: string }>(props: {
  items: T[];
  label: (item: T) => string;
  icon?: (item: T) => IconName;
  checked: (item: T) => boolean;
  onToggle: (item: T) => void;
}) {
  return (
    <Show when={props.items.length > 0}>
      <div class="flex flex-col divide-y divide-phone-separator overflow-hidden rounded-[10px] bg-phone-row">
        {props.items.map((item) => (
          <button
            type="button"
            class="flex min-h-[44px] w-full items-center gap-3 px-4 py-2 text-left text-[17px]"
            onClick={() => props.onToggle(item)}
          >
            <Show when={props.icon}>
              {(icon) => <Icon name={icon()(item)} class="size-[18px] shrink-0 text-phone-tint" />}
            </Show>
            <span class="min-w-0 flex-1 truncate">{props.label(item)}</span>
            {props.checked(item) ? <Icon name="check" class="size-[22px] text-phone-tint" /> : null}
          </button>
        ))}
      </div>
    </Show>
  );
}

export function InlineField(props: {
  value: string;
  placeholder: string;
  onInput: (value: string) => void;
  error?: string | null;
  header?: string;
}) {
  return (
    <div class="flex flex-col">
      {props.header ? (
        <span class="px-4 pb-1.5 text-[13px] text-phone-border uppercase">{props.header}</span>
      ) : null}
      <div class="overflow-hidden rounded-[10px] bg-phone-row">
        <input
          type="text"
          class="flex min-h-[44px] w-full bg-transparent px-4 py-2 text-[17px] outline-none placeholder:text-phone-border"
          value={props.value}
          placeholder={props.placeholder}
          autocapitalize="off"
          spellcheck={false}
          onInput={(event) => props.onInput(event.currentTarget.value)}
        />
      </div>
      {props.error ? (
        <span class="px-4 pt-1.5 text-[13px] font-bold text-phone-live">{props.error}</span>
      ) : null}
    </div>
  );
}
