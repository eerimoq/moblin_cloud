/* @refresh reload */
import { render } from "solid-js/web";
import "./index.css";
import NotFound from "./pages/NotFound";
import SongRequest from "./pages/SongRequest";

const twitchLogin = location.pathname
  .slice(import.meta.env.BASE_URL.length)
  .match(/^twitch\/([^/]+)\/?$/)?.[1];

if (twitchLogin) {
  document.title = "Song requests – Moblin";
}

render(
  () => (twitchLogin ? <SongRequest twitch={decodeURIComponent(twitchLogin)} /> : <NotFound />),
  document.getElementById("root")!,
);
