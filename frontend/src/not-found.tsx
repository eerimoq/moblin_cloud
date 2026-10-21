/* @refresh reload */
import { lazy } from "solid-js";
import { render } from "solid-js/web";
import "./index.css";
import { t } from "./i18n";
import NotFound from "./pages/NotFound";

const Page = lazy(() => import("./pages/Page"));
const DashboardPage = lazy(() => import("./pages/DashboardPage"));

const path = location.pathname.slice(import.meta.env.BASE_URL.length);
const twitchLogin = path.match(/^twitch\/([^/]+)\/?$/)?.[1];
const kickLogin = path.match(/^kick\/([^/]+)\/?$/)?.[1];
const dashboard = /^dashboard\/twitch\/[^/]+\/?$/.test(path);

if (twitchLogin || kickLogin) {
  document.title = "Moblin";
} else if (dashboard) {
  document.title = t("Moblin Cloud – Moblin");
}

render(
  () =>
    twitchLogin ? (
      <Page twitch={decodeURIComponent(twitchLogin)} />
    ) : kickLogin ? (
      <Page kick={decodeURIComponent(kickLogin)} />
    ) : dashboard ? (
      <DashboardPage />
    ) : (
      <NotFound />
    ),
  document.getElementById("root")!,
);
