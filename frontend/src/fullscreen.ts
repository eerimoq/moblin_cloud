import { createSignal } from "solid-js";

const [fullscreen, setFullscreen] = createSignal(false);
let browser = false;
document.addEventListener("fullscreenchange", () => {
  if (document.fullscreenElement === null && browser) {
    browser = false;
    setFullscreen(false);
  }
});

export { fullscreen };

export const toggleFullscreen = () => {
  if (fullscreen()) {
    setFullscreen(false);
    if (browser) {
      browser = false;
      void document.exitFullscreen().catch(() => {});
    }
  } else {
    setFullscreen(true);
    if (document.fullscreenEnabled) {
      browser = true;
      void document.documentElement.requestFullscreen({ navigationUI: "hide" }).catch(() => {
        browser = false;
      });
    }
  }
};
