/* @refresh reload */
import { lazy } from "solid-js";
import { render } from "solid-js/web";
import "./index.css";
import { t } from "./i18n";
import NotFound from "./pages/NotFound";

const Page = lazy(() => import("./pages/Page"));
const DashboardPage = lazy(() => import("./pages/DashboardPage"));
const DevicePage = lazy(() => import("./pages/Device"));

const path = location.pathname.slice(import.meta.env.BASE_URL.length);
const streamer = path.match(/^(twitch|kick)\/([^/]+)\/?$/);
const dashboard = /^dashboard\/(twitch|kick)\/[^/]+\/?$/.test(path);
const device = path.match(/^device\/([^/]+)(?:\/([^/]+))?\/?$/);

if (streamer) {
  document.title = "Moblin";
} else if (dashboard) {
  document.title = t("Moblin Cloud – Moblin");
} else if (device) {
  document.title = t("Device – Moblin");
}

render(
  () =>
    streamer ? (
      <Page platform={streamer[1] as "twitch" | "kick"} login={decodeURIComponent(streamer[2])} />
    ) : dashboard ? (
      <DashboardPage />
    ) : device ? (
      <DevicePage
        page={decodeURIComponent(device[1])}
        device={decodeURIComponent(device[2] ?? "")}
      />
    ) : (
      <NotFound />
    ),
  document.getElementById("root")!,
);
