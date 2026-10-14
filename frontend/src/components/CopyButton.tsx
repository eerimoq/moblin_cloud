import { createSignal } from "solid-js";
import Icon from "./Icon";

function copyWithExecCommand(text: string): boolean {
  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.setAttribute("readonly", "");
  textarea.style.position = "fixed";
  textarea.style.opacity = "0";
  document.body.appendChild(textarea);
  textarea.select();
  let ok = false;
  try {
    ok = document.execCommand("copy");
  } catch {
    ok = false;
  }
  textarea.remove();
  return ok;
}

async function copyText(text: string): Promise<boolean> {
  if (navigator.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(text);
      return true;
    } catch {}
  }
  return copyWithExecCommand(text);
}

export default function CopyButton(props: { text: string; label?: string }) {
  const [copied, setCopied] = createSignal(false);
  let timer: ReturnType<typeof setTimeout> | undefined;

  const copy = async () => {
    if (!(await copyText(props.text))) {
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
