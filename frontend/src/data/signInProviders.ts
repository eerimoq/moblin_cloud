import { createSignal, onMount } from "solid-js";
import { call } from "./backend";
import type { SignInProviders } from "./frontendConfig";

const [providers, setProviders] = createSignal<SignInProviders | null>(null);
let pending: Promise<void> | undefined;

export function useSignInProviders() {
  onMount(() => {
    pending ??= call<SignInProviders>("/auth/providers")
      .then((value) => {
        setProviders(value);
      })
      .catch(() => {
        setProviders({ twitch: false, kick: false });
      });
  });
  return providers;
}
