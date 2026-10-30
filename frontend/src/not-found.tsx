/* @refresh reload */
import { lazy } from "solid-js";
import { render } from "solid-js/web";
import "./index.css";
import { t } from "./i18n";
import NotFound from "./pages/NotFound";

const Page = lazy(() => import("./pages/Page"));
const DashboardPage = lazy(() => import("./pages/DashboardPage"));

const path = location.pathname.slice(import.meta.env.BASE_URL.length);
const streamer = path.match(/^(twitch|kick)\/([^/]+)\/?$/);
const dashboard =
  /^(dashboard\/(twitch|kick)\/[^/]+|device\/[^/]+(\/[^/]+)?|monitor\/[^/]+)\/?$/.test(path);

if (streamer) {
  document.title = "Moblin";
} else if (dashboard) {
  document.title = t("Moblin Cloud – Moblin");
}

render(
  () =>
    streamer ? (
      <Page platform={streamer[1] as "twitch" | "kick"} login={decodeURIComponent(streamer[2])} />
    ) : dashboard ? (
      <DashboardPage />
    ) : (
      <NotFound />
    ),
  document.getElementById("root")!,
);
