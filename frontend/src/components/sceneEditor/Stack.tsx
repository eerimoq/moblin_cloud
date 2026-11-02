import { createSignal, Show, type Accessor, type JSX } from "solid-js";
import Icon from "../Icon";

export type Page = { title: string | (() => string); render: () => JSX.Element };

export const pageTitle = (page: Page) =>
  typeof page.title === "function" ? page.title() : page.title;

export type Stack = {
  pages: Accessor<Page[]>;
  push: (page: Page) => void;
  pop: () => void;
  reset: () => void;
};

export function createStack(): Stack {
  const [pages, setPages] = createSignal<Page[]>([]);
  return {
    pages,
    push: (page) => setPages((current) => [...current, page]),
    pop: () => setPages((current) => current.slice(0, -1)),
    reset: () => setPages([]),
  };
}

export function NavigationView(props: {
  stack: Stack;
  root: Page;
  trailing?: JSX.Element;
  children?: JSX.Element;
}) {
  const top = () => props.stack.pages().at(-1) ?? props.root;
  const previous = () => {
    const pages = props.stack.pages();
    return pages.length === 0
      ? undefined
      : pages.length === 1
        ? props.root
        : pages[pages.length - 2];
  };
  return (
    <div class="flex h-full w-full flex-col bg-phone-screen">
      <div class="relative flex h-[44px] shrink-0 items-center justify-center px-[110px]">
        <Show when={previous()}>
          {(page) => (
            <button
              type="button"
              class="absolute left-[8px] flex max-w-[100px] items-center gap-0.5 text-[17px] text-phone-tint"
              onClick={() => props.stack.pop()}
            >
              <Icon name="chevron-left" class="size-[20px] shrink-0" />
              <span class="truncate">{pageTitle(page())}</span>
            </button>
          )}
        </Show>
        <span class="truncate text-[17px] font-semibold">{pageTitle(top())}</span>
        <Show when={props.trailing}>
          <div class="absolute right-[6px]">{props.trailing}</div>
        </Show>
      </div>
      <div class="flex min-h-0 flex-1 flex-col gap-6 overflow-y-auto px-4 pb-6 [scrollbar-width:none]">
        <Show when={top()} keyed>
          {(page) => page.render()}
        </Show>
      </div>
      {props.children}
    </div>
  );
}
