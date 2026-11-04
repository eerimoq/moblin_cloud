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

const query = (media: string) => {
  const list = matchMedia(media);
  const [matches, setMatches] = createSignal(list.matches);
  list.addEventListener("change", () => setMatches(list.matches));
  return matches;
};

export const wide = query("(min-width: 768px)");
export const small = query("(max-width: 500px), (max-height: 500px)");

export const fullscreenOffered = () => !small() || document.fullscreenEnabled;

const [scrolledDown, setScrolledDown] = createSignal(false);
let turn = 0;
addEventListener(
  "scroll",
  () => {
    const y = Math.max(0, scrollY);
    if (scrolledDown()) {
      if (y > turn) {
        turn = y;
      } else if (y === 0 || y < turn - 10) {
        turn = y;
        setScrolledDown(false);
      }
    } else if (y < turn) {
      turn = y;
    } else if (y > turn + 10) {
      turn = y;
      setScrolledDown(true);
    }
  },
  { passive: true },
);

export { scrolledDown };
