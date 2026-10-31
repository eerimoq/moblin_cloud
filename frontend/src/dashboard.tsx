/* @refresh reload */
import { render } from "solid-js/web";
import "./index.css";
import DashboardPage from "./pages/DashboardPage";
import { connectPendingKey } from "./data/account";

if (location.pathname === `${import.meta.env.BASE_URL}connect/`) {
  try {
    sessionStorage.setItem(connectPendingKey, "1");
  } catch {}
}

if (location.pathname === `${import.meta.env.BASE_URL}dashboard/`) {
  history.replaceState(null, "", `${import.meta.env.BASE_URL}${location.search}${location.hash}`);
}

render(() => <DashboardPage />, document.getElementById("root")!);
