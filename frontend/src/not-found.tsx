/* @refresh reload */
import { render } from "solid-js/web";
import "./index.css";
import NotFound from "./pages/NotFound";
import SongRequest from "./pages/SongRequest";

const songRequestName = location.pathname
  .slice(import.meta.env.BASE_URL.length)
  .match(/^song-requests\/([^/]+)\/?$/)?.[1];

if (songRequestName) {
  document.title = "Song requests – Moblin";
}

render(
  () =>
    songRequestName ? <SongRequest name={decodeURIComponent(songRequestName)} /> : <NotFound />,
  document.getElementById("root")!,
);
