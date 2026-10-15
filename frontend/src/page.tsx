/* @refresh reload */
import { render } from "solid-js/web";
import "./index.css";
import { asset } from "./asset";
import { pages } from "./data/links";
import Page from "./pages/Page";

if (new URLSearchParams(location.search).has("streamer")) {
  render(() => <Page />, document.getElementById("root")!);
} else {
  location.replace(asset(pages.dashboard) + location.hash);
}
