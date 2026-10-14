import { createSignal } from "solid-js";
import Icon from "./Icon";

export default function CopyButton(props: { text: string; label?: string }) {
  const [copied, setCopied] = createSignal(false);
  let timer: ReturnType<typeof setTimeout> | undefined;

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(props.text);
    } catch {
      return;
    }
    setCopied(true);
    clearTimeout(timer);
    timer = setTimeout(() => setCopied(false), 1500);
  };

  return (
    <button
      type="button"
      class={`inline-flex size-7 shrink-0 items-center justify-center rounded-md transition-colors ${
        copied() ? "text-leaf" : "text-muted hover:bg-ghost hover:text-ink"
      }`}
      aria-label={copied() ? "Copied" : `Copy ${props.label ?? props.text}`}
      title={copied() ? "Copied" : "Copy"}
      onClick={copy}
    >
      <Icon name={copied() ? "check" : "copy"} class="size-4" />
    </button>
  );
}
