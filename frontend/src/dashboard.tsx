/* @refresh reload */
import { render } from "solid-js/web";
import "./index.css";
import DashboardPage from "./pages/DashboardPage";

if (location.pathname === `${import.meta.env.BASE_URL}dashboard/`) {
  history.replaceState(null, "", `${import.meta.env.BASE_URL}${location.search}${location.hash}`);
}

render(() => <DashboardPage />, document.getElementById("root")!);
