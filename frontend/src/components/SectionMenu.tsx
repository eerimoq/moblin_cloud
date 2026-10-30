import { createSignal, For, onCleanup, Show } from "solid-js";
import Icon, { type IconName } from "./Icon";

export type Section = {
  id: string;
  label: string;
  icon: IconName;
  status: () => { on: boolean; text: string } | null;
};

export function createSectionChoice(visible: () => Section[], initial = location.hash.slice(1)) {
  const [chosen, setChosen] = createSignal(initial);
  const active = () =>
    visible().some((section) => section.id === chosen()) ? chosen() : visible()[0]!.id;
  const select = (id: string) => {
    const url = `${location.pathname}${location.search}#${id}`;
    if (id === active()) {
      history.replaceState(null, "", url);
    } else {
      scrollTo({ top: 0, behavior: "instant" });
      history.pushState(null, "", url);
    }
    setChosen(id);
  };
  const onPopState = () => {
    scrollTo({ top: 0, behavior: "instant" });
    setChosen(location.hash.slice(1));
  };
  addEventListener("popstate", onPopState);
  onCleanup(() => removeEventListener("popstate", onPopState));
  return { active, select };
}

function Status(props: { status: { on: boolean; text: string } }) {
  return (
    <span class="ml-auto flex items-center gap-1.5 pl-1 text-[13px] text-muted">
      <span class={`size-2 shrink-0 rounded-full ${props.status.on ? "bg-leaf" : "bg-muted/50"}`} />
      {props.status.text}
    </span>
  );
}

export default function SectionMenu(props: {
  label: string;
  sections: Section[];
  active: string;
  onSelect: (id: string) => void;
}) {
  const [open, setOpen] = createSignal(false);
  const current = () => props.sections.find((section) => section.id === props.active)!;

  return (
    <nav
      aria-label={props.label}
      class="overflow-hidden rounded-2xl border border-line bg-card lg:sticky lg:top-[calc(3.5rem+1px+3rem)] lg:flex lg:max-h-[calc(100dvh-3.5rem-1px-6rem)] lg:flex-col lg:overflow-y-auto lg:rounded-none lg:border-0 lg:bg-transparent"
    >
      <button
        type="button"
        aria-expanded={open()}
        onClick={() => setOpen(!open())}
        class="flex w-full items-center gap-3 px-4 py-3 text-left text-ink lg:hidden"
      >
        <Icon name={current().icon} class="size-5 shrink-0" />
        <span class="font-display text-[17px] font-semibold">{current().label}</span>
        <Show when={current().status()}>{(status) => <Status status={status()} />}</Show>
        <Icon
          name="chevron-down"
          class={`size-5 shrink-0 text-muted transition-transform ${open() ? "rotate-180" : ""} ${current().status() ? "" : "ml-auto"}`}
        />
      </button>
      <div
        class="divide-line flex flex-col divide-y border-t border-line lg:contents"
        classList={{ hidden: !open() }}
      >
        <For each={props.sections}>
          {(section) => (
            <button
              type="button"
              aria-current={props.active === section.id ? "page" : undefined}
              onClick={() => {
                props.onSelect(section.id);
                setOpen(false);
              }}
              class="flex items-center gap-3 px-4 py-2.5 text-left transition-colors lg:rounded-2xl lg:border-[1.5px] lg:py-3 lg:whitespace-nowrap"
              classList={{
                "bg-ghost text-ink lg:border-line lg:bg-card": props.active === section.id,
                "text-ink-2 hover:bg-ghost-hover hover:text-ink lg:border-transparent lg:hover:bg-ghost":
                  props.active !== section.id,
              }}
            >
              <Icon name={section.icon} class="size-5 shrink-0" />
              <span class="font-display text-[16px] font-semibold lg:text-[17px]">
                {section.label}
              </span>
              <Show when={section.status()}>{(status) => <Status status={status()} />}</Show>
            </button>
          )}
        </For>
      </div>
    </nav>
  );
}
