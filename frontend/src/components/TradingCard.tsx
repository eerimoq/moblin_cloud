import { For, Show } from "solid-js";
import { cardArt } from "../cardArt";
import { rarities, type Card } from "../data/cards";

type Props = {
  card: Card;
  /** When given, the card is a button that opens itself enlarged. */
  onOpen?: (card: Card) => void;
};

export default function TradingCard(props: Props) {
  const rarity = () => rarities.find((r) => r.key === props.card.rarity)!;
  const interactive = () => props.onOpen !== undefined;

  return (
    <article
      class={`tcard tcard-${props.card.rarity}${props.card.full ? " tcard-full" : ""}${interactive() ? " tcard-button" : ""}`}
      tabIndex={interactive() ? 0 : undefined}
      role={interactive() ? "button" : undefined}
      data-card={props.card.id}
      aria-label={`${props.card.name}, ${rarity().label}, power ${props.card.power}`}
      onClick={(e) => {
        if (!(e.target as HTMLElement).closest("a")) {
          props.onOpen?.(props.card);
        }
      }}
      onKeyDown={(e) => {
        if (interactive() && (e.key === "Enter" || e.key === " ")) {
          e.preventDefault();
          props.onOpen?.(props.card);
        }
      }}
    >
      <div class="tcard-name">
        <h3>{props.card.name}</h3>
        <span class="tcard-power" title="Power">
          {props.card.power}
        </span>
      </div>
      <div class="tcard-art" innerHTML={cardArt[props.card.id]()} />
      <Show when={props.card.full}>
        <div class="tcard-spacer" />
      </Show>
      <div class="tcard-type">
        <span>
          {rarity().label} · {props.card.subtype}
        </span>
        <span class="tcard-sym" />
      </div>
      <div class="tcard-text">
        <div>{props.card.text}</div>
        <Show when={props.card.links}>
          {(cardLinks) => (
            <div class="tcard-links">
              <For each={cardLinks()}>
                {(l) => (
                  <a href={l.href} target="_blank" rel="noopener">
                    {l.label}
                  </a>
                )}
              </For>
            </div>
          )}
        </Show>
      </div>
      <div class="tcard-foot">
        <div class="tcard-credit">
          <b>{props.card.credit}</b>
          <Show when={props.card.also}>
            <small>{props.card.also}</small>
          </Show>
        </div>
        <div class="tcard-set">{props.card.version}</div>
      </div>
    </article>
  );
}
