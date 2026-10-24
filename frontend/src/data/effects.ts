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

const effectCredits: Record<string, { title: string; author: string; url: string }> = {
  fart: { title: "FART.aif", author: "Manicciola", url: "https://freesound.org/s/121783/" },
  "fart-2": {
    title: "Fart sound.wav",
    author: "aditwayer",
    url: "https://freesound.org/s/520671/",
  },
  "bad-chili-fart": {
    title: "Bad Chili Fart.wav",
    author: "deleted_user_1391979",
    url: "https://freesound.org/s/94989/",
  },
  "perfect-fart": {
    title: "perfect-fart.mp3",
    author: "TV_LING",
    url: "https://freesound.org/s/523467/",
  },
};

export const effectCredit = (id: string): (typeof effectCredits)[string] | undefined =>
  effectCredits[id];

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
