import { For, Show } from "solid-js";
import { asset } from "../asset";
import type { Product } from "../data/products";
import { fill } from "../i18n";
import Button from "./Button";
import Icon from "./Icon";

export default function ProductCard(props: { product: Product }) {
  const p = () => props.product;
  return (
    <li class="flex flex-col gap-5 rounded-3xl border-[1.5px] border-line bg-card p-6 sm:p-9">
      <div class="flex items-center gap-5">
        <Show
          when={p().logo}
          fallback={
            <div class="flex size-[84px] shrink-0 items-center justify-center">
              <Show when={p().icon}>
                {(icon) => <Icon name={icon()} class="size-14 text-leaf [stroke-width:1.6]" />}
              </Show>
            </div>
          }
        >
          {(logo) => (
            <img
              src={asset(logo())}
              alt=""
              class="h-[90px] w-[84px] shrink-0 object-contain"
              loading="lazy"
            />
          )}
        </Show>
        <div class="flex flex-col gap-1">
          <h3 class="text-2xl font-bold sm:text-[30px]">{p().name}</h3>
          <p class="font-bold text-leaf">{p().tagline}</p>
        </div>
      </div>
      <p class="text-ink-2">{p().description}</p>
      <ol class="flex flex-col gap-2.5 rounded-2xl bg-page px-5 py-4">
        <For each={p().steps}>
          {(step) => (
            <li class="text-sm text-ink-2">
              <Show when={step.lead}>
                <strong class="font-bold text-ink">{step.lead} </strong>
              </Show>
              {fill(step.text, {
                code: (
                  <code class="rounded-lg border border-line bg-well px-2 py-0.5 font-mono text-[13.5px] whitespace-nowrap text-ink">
                    {step.code}
                  </code>
                ),
                path: <strong class="font-bold text-ink">{step.path}</strong>,
              })}
            </li>
          )}
        </For>
      </ol>
      <div class="mt-1 flex flex-wrap gap-3">
        <Show
          when={p().badge}
          fallback={
            <Button href={p().primary.href} size="sm">
              <Show when={p().primary.icon}>{(icon) => <Icon name={icon()} class="size-5" />}</Show>
              {p().primary.label}
            </Button>
          }
        >
          {(badge) => (
            <a href={badge().href} target="_blank" rel="noopener" class="flex">
              <img src={asset(badge().src)} alt={badge().alt} class="h-11 w-auto" />
            </a>
          )}
        </Show>
        <Button href={p().secondary.href} variant="ghost" size="sm">
          <Show when={p().secondary.icon}>{(icon) => <Icon name={icon()} class="size-5" />}</Show>
          {p().secondary.label}
        </Button>
      </div>
    </li>
  );
}
