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

export const playEffect = (id: string) => {
  playing?.pause();
  playing = new Audio(`${backendUrl}/effects/${id}`);
  playing.play().catch(() => undefined);
};
