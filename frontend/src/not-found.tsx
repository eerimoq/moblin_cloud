/* @refresh reload */
import { lazy } from "solid-js";
import { render } from "solid-js/web";
import "./index.css";
import NotFound from "./pages/NotFound";

const Page = lazy(() => import("./pages/Page"));

const twitchLogin = location.pathname
  .slice(import.meta.env.BASE_URL.length)
  .match(/^twitch\/([^/]+)\/?$/)?.[1];

if (twitchLogin) {
  document.title = "Moblin";
}

render(
  () => (twitchLogin ? <Page twitch={decodeURIComponent(twitchLogin)} /> : <NotFound />),
  document.getElementById("root")!,
);
