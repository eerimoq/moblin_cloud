/* @refresh reload */
import { render } from "@solidjs/web";
import "./index.css";
import { asset } from "./asset";
import { pages } from "./data/links";

if (new URLSearchParams(location.search).has("streamer")) {
  const { default: Page } = await import("./pages/Page");
  render(() => <Page />, document.getElementById("root")!);
} else {
  location.replace(asset(pages.dashboard) + location.hash);
}
