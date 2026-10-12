import { cardBack } from "../cardArt";

type Props = {
  /** When given, the back is a button that opens itself enlarged. */
  onOpen?: () => void;
};

export default function CardBack(props: Props) {
  const interactive = () => props.onOpen !== undefined;

  return (
    <div
      class={`tcard tcard-back${interactive() ? " tcard-button" : ""}`}
      role={interactive() ? "button" : "img"}
      tabIndex={interactive() ? 0 : undefined}
      aria-label="Card back"
      innerHTML={cardBack()}
      onClick={() => props.onOpen?.()}
      onKeyDown={(e) => {
        if (interactive() && (e.key === "Enter" || e.key === " ")) {
          e.preventDefault();
          props.onOpen?.();
        }
      }}
    />
  );
}
