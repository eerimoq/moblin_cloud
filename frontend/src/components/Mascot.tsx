import { asset } from "../asset";

type MascotVariant = "party" | "looking";

type Props = {
  variant: MascotVariant;
  class?: string;
  alt?: string;
};

export default function Mascot(props: Props) {
  return (
    <img
      src={asset(`logos/logo-${props.variant}.png`)}
      alt={props.alt ?? ""}
      class={props.class ?? ""}
      loading="lazy"
      decoding="async"
    />
  );
}
