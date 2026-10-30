import Icon from "./Icon";

type BrandName = "twitch" | "youtube" | "kick" | "facebook" | "obs";

/** Each brand's colors, matching the logos the Moblin app ships with. */
const brandColors: Record<BrandName, { background: string; color: string }> = {
  twitch: { background: "#9146ff", color: "#ffffff" },
  youtube: { background: "#ff0033", color: "#ffffff" },
  kick: { background: "#000000", color: "#53fc18" },
  facebook: { background: "#0866ff", color: "#ffffff" },
  obs: { background: "#302e31", color: "#ffffff" },
};

type Props = {
  name: BrandName;
  /** Sets the disc size; the mark scales with it. */
  class?: string;
};

/** A brand's mark on a round disc in the brand's own colors. */
export default function BrandIcon(props: Props) {
  return (
    <span
      class={`inline-flex shrink-0 items-center justify-center rounded-full ${props.class ?? "size-6"}`}
      style={brandColors[props.name]}
      aria-hidden="true"
    >
      <Icon name={props.name} class="size-[62%]" />
    </span>
  );
}
