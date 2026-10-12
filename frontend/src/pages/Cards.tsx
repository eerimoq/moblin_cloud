import { createSignal, For, onCleanup, onMount, Show } from "solid-js";
import CardBack from "../components/CardBack";
import Footer from "../components/Footer";
import Mascot from "../components/Mascot";
import Nav from "../components/Nav";
import TradingCard from "../components/TradingCard";
import { cards, rarities, type Card } from "../data/cards";
import { links } from "../data/links";

export default function Cards() {
  const [selected, setSelected] = createSignal<Card | "back" | null>(null);
  const [zoom, setZoom] = createSignal(1.5);

  const open = (card: Card | "back") => {
    setZoom(Math.min(1.5, (innerWidth - 32) / 284, (innerHeight - 32) / 396));
    setSelected(card);
  };

  onMount(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setSelected(null);
      }
    };
    addEventListener("keydown", onKey);
    onCleanup(() => removeEventListener("keydown", onKey));
  });

  return (
    <>
      <Nav />
      <main>
        <section class="mx-auto flex max-w-[1120px] flex-col gap-6 px-5 pt-12 pb-10 sm:px-8 lg:pt-18">
          <div class="flex items-start justify-between gap-6">
            <div class="flex max-w-[640px] flex-col gap-4">
              <span class="font-display text-[15px] font-semibold uppercase tracking-[0.08em] text-leaf">
                Trading cards
              </span>
              <h1 class="text-[40px] leading-[1.05] font-bold text-balance sm:text-6xl">
                Collect the whole deck.
              </h1>
              <p class="text-lg text-ink-2 text-pretty sm:text-[21px]">
                Every Moblin feature as a trading card, sorted by rarity. The number in the corner
                says how good the card is, out of 10. One card refuses the scale.
              </p>
              <p class="text-[15px] text-muted">
                The 💡 line credits whoever asked for the feature, as recorded in Moblin's version
                history, with the version it shipped in. Where nobody asked, it just appeared.
              </p>
            </div>
            <Mascot
              variant="party"
              class="hidden w-[150px] shrink-0 rotate-[6deg] sm:block lg:w-[190px]"
            />
          </div>
        </section>

        <section class="bg-band py-16 lg:py-20">
          <div class="mx-auto flex max-w-[1120px] flex-col gap-10 px-5 sm:px-8">
            <For each={rarities}>
              {(rarity) => {
                const mine = cards.filter((c) => c.rarity === rarity.key);
                return (
                  <section class="flex flex-col gap-4">
                    <div class="flex flex-col gap-1">
                      <h2 class="flex items-baseline gap-3 text-[23px] font-semibold">
                        {rarity.label}
                        <span class="font-mono text-xs font-normal text-muted">
                          {mine.length} {mine.length === 1 ? "card" : "cards"}
                        </span>
                      </h2>
                      <p class="max-w-[70ch] text-[15px] text-muted">{rarity.note}</p>
                    </div>
                    <div class="flex flex-wrap gap-5 max-[420px]:justify-center">
                      <For each={mine}>{(card) => <TradingCard card={card} onOpen={open} />}</For>
                    </div>
                  </section>
                );
              }}
            </For>
            <section class="flex flex-col gap-4">
              <div class="flex flex-col gap-1">
                <h2 class="text-[23px] font-semibold">The back</h2>
                <p class="max-w-[70ch] text-[15px] text-muted">
                  Every card has the same back. Shuffle with confidence.
                </p>
              </div>
              <div class="flex flex-wrap gap-5 max-[420px]:justify-center">
                <CardBack onOpen={() => open("back")} />
              </div>
            </section>
          </div>
        </section>

        <section class="mx-auto max-w-[1120px] px-5 py-12 sm:px-8">
          <p class="text-[15px] text-muted">
            Descriptions and ratings are affectionate exaggerations. The rules of the game are left
            as an exercise for{" "}
            <a
              href={links.discord}
              target="_blank"
              rel="noopener"
              class="text-leaf hover:text-leaf-bright"
            >
              Discord
            </a>
            .
          </p>
        </section>
      </main>
      <Footer />
      <Show when={selected()}>
        {(card) => (
          <div
            class="tcard-lightbox"
            style={{ "--zoom": zoom().toFixed(2) }}
            onClick={(e) => {
              if (!(e.target as HTMLElement).closest("a")) {
                setSelected(null);
              }
            }}
          >
            <Show when={card() !== "back"} fallback={<CardBack />}>
              <TradingCard card={card() as Card} />
            </Show>
          </div>
        )}
      </Show>
    </>
  );
}
