import { For } from "solid-js";
import { products } from "../data/products";
import ProductCard from "./ProductCard";
import { t } from "../i18n";

export default function Ecosystem() {
  return (
    <section id="ecosystem" class="pt-10 pb-24">
      <div class="mx-auto flex max-w-[1120px] flex-col gap-10 px-5 sm:px-8">
        <div class="flex flex-col gap-4 md:flex-row md:items-end md:justify-between md:gap-6">
          <div class="flex max-w-[640px] flex-col gap-2.5">
            <span class="font-display text-[15px] font-semibold uppercase tracking-[0.08em] text-leaf">
              {t("The Moblin family")}
            </span>
            <h2 class="text-4xl font-bold sm:text-[46px]">
              {t("More green friends for your setup.")}
            </h2>
          </div>
          <p class="max-w-[440px] text-lg text-muted">
            {t("Free companions that plug into Moblin. Pick what you need, skip what you don't.")}
          </p>
        </div>
        <ul class="grid gap-6 lg:grid-cols-2">
          <For each={products}>{(product) => <ProductCard product={product} />}</For>
        </ul>
      </div>
    </section>
  );
}
