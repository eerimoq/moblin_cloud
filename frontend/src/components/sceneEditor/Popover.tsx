import { createEffect, Show } from "solid-js";
import type { JSX } from "@solidjs/web";
import Icon, { type IconName } from "../Icon";

export function Popover(props: {
  open: boolean;
  onClose: () => void;
  side?: "left" | "above";
  children: JSX.Element;
}) {
  let card: HTMLDivElement | undefined;
  createEffect(
    () => props.open,
    (open) => {
      if (!open) {
        return;
      }
      const onPointerDown = (event: PointerEvent) => {
        if (card && !card.contains(event.target as Node)) {
          props.onClose();
        }
      };
      const onKey = (event: KeyboardEvent) => {
        if (event.key === "Escape") {
          props.onClose();
        }
      };
      const timer = setTimeout(() => {
        document.addEventListener("pointerdown", onPointerDown, true);
        document.addEventListener("keydown", onKey);
      });
      return () => {
        clearTimeout(timer);
        document.removeEventListener("pointerdown", onPointerDown, true);
        document.removeEventListener("keydown", onKey);
      };
    },
  );
  return (
    <Show when={props.open}>
      <div
        ref={(element) => (card = element)}
        class={[
          "absolute z-30 rounded-[13px] bg-phone-row text-white shadow-[0_8px_30px_rgba(0,0,0,0.6)]",
          props.side === "above" ? "bottom-0 left-0" : "top-0 right-0",
        ]}
        onPointerDown={(event) => event.stopPropagation()}
      >
        {props.children}
      </div>
    </Show>
  );
}

export function PopoverList(props: { children: JSX.Element }) {
  return (
    <div class="flex max-h-[360px] min-w-[220px] flex-col overflow-y-auto p-[5px] [scrollbar-width:none]">
      {props.children}
    </div>
  );
}

export function PopoverRow(props: {
  icon: IconName;
  text: string;
  checked?: boolean;
  tinted?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      class={[
        "flex items-center gap-[8px] p-[11px] text-left text-[17px]",
        props.tinted ? "text-phone-tint" : "text-white",
      ]}
      onClick={() => props.onClick()}
    >
      <Icon name={props.icon} class="size-[20px] w-[32px] shrink-0" />
      <span class="min-w-0 flex-1 truncate">{props.text}</span>
      <Show when={props.checked}>
        <Icon name="check" class="size-[18px] shrink-0" />
      </Show>
    </button>
  );
}

export function PopoverTitle(props: { text: string }) {
  return (
    <span class="px-[11px] pt-[11px] pb-[5px] text-[13px] text-phone-border">{props.text}</span>
  );
}

export function PopoverDivider() {
  return <div class="my-[2px] h-px bg-phone-separator" />;
}
