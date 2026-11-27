import { createSignal } from "solid-js";
import { backendUrl } from "./backend";
import { t } from "../i18n";

const effectNames: Record<string, string> = {
  fart: t("Fart"),
  "fart-2": t("Fart 2"),
  "bad-chili-fart": t("Bad chili fart"),
  "perfect-fart": t("Perfect fart"),
  "notification-2": t("Notification 2"),
  boing: t("Boing"),
  "cash-register": t("Cash register"),
  dingaling: t("Dingaling"),
  "level-up": t("Level up"),
  notification: t("Notification"),
  "sfx-magic": t("SFX magic"),
  whoosh: t("Whoosh"),
  "coin-dropping": t("Coin dropping"),
};

export const effects = Object.keys(effectNames);

export const isFart = (id: string) => id.includes("fart");

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
  "notification-2": {
    title: "notification2-freesound.wav",
    author: "Thoribass",
    url: "https://freesound.org/s/254819/",
  },
  boing: { title: "Boing.wav", author: "juskiddink", url: "https://freesound.org/s/140867/" },
  "cash-register": {
    title: "Cash Register",
    author: "kiddpark",
    url: "https://freesound.org/s/201159/",
  },
  dingaling: { title: "dingaling", author: "morrisjm", url: "https://freesound.org/s/268756/" },
  "level-up": {
    title: "320655__rhodesmas__level-up-01.mp3",
    author: "shinephoenixstormcrow",
    url: "https://freesound.org/s/337049/",
  },
  notification: {
    title: "Message Notification 4",
    author: "AnthonyRox",
    url: "https://freesound.org/s/740423/",
  },
  "sfx-magic": { title: "SFX Magic", author: "renatalmar", url: "https://freesound.org/s/264981/" },
  whoosh: { title: "Whoosh", author: "qubodup", url: "https://freesound.org/s/60013/" },
  "coin-dropping": {
    title: "Coin dropping.wav",
    author: "Jace",
    url: "https://freesound.org/s/17502/",
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
