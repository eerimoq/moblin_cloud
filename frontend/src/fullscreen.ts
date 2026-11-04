import { createSignal } from "solid-js";

const [fullscreen, setFullscreen] = createSignal(document.fullscreenElement !== null);
document.addEventListener("fullscreenchange", () =>
  setFullscreen(document.fullscreenElement !== null),
);

export { fullscreen };

export const fullscreenAvailable = document.fullscreenEnabled;

export const toggleFullscreen = () =>
  (fullscreen()
    ? document.exitFullscreen()
    : document.documentElement.requestFullscreen({ navigationUI: "hide" })
  ).catch(() => {});
