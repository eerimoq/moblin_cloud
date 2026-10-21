import { createSignal, onCleanup, onMount } from "solid-js";
import { backendUrl } from "./data/backend";
import { poll } from "./poll";

/**
 * Whether the Twitch channel the backend keeps an eye on (Erik's) is live
 * right now. The backend asks Twitch, since that needs a secret a static site
 * cannot hold. Falls back to "not live" whenever the check fails.
 */
export function createTwitchLive(refreshMs = 5 * 60 * 1000) {
  const [live, setLive] = createSignal(false);

  const check = async () => {
    try {
      const res = await fetch(`${backendUrl}/twitch/live`, { cache: "no-store" });
      if (!res.ok) throw new Error(`checking Twitch live status failed with status ${res.status}`);
      const body: { live: boolean } = await res.json();
      setLive(body.live === true);
    } catch {
      setLive(false);
    }
  };

  onMount(() => {
    void check();
    onCleanup(poll(() => void check(), refreshMs));
  });

  return live;
}
