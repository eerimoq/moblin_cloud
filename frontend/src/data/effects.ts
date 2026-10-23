import { createSignal } from "solid-js";
import { backendUrl } from "./backend";
import { t } from "../i18n";

const effectNames: Record<string, string> = {
  fart: t("Fart"),
  "fart-2": t("Fart 2"),
  "bad-chili-fart": t("Bad chili fart"),
  "perfect-fart": t("Perfect fart"),
};

export const effects = Object.keys(effectNames);

export const effectName = (id: string) => effectNames[id] ?? id;

let playing: HTMLAudioElement | null = null;

const [playingEffect, setPlayingEffect] = createSignal<string | null>(null);

export { playingEffect };

export const stopEffect = () => {
  playing?.pause();
  playing = null;
  setPlayingEffect(null);
};

export const playEffect = (id: string) => {
  stopEffect();
  const audio = new Audio(`${backendUrl}/effects/${id}`);
  const ended = () => {
    if (playing === audio) {
      stopEffect();
    }
  };
  audio.addEventListener("ended", ended);
  audio.addEventListener("error", ended);
  playing = audio;
  setPlayingEffect(id);
  audio.play().catch(ended);
};
