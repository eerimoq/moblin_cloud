/* @refresh reload */
import { render } from "solid-js/web";
import "./index.css";
import Footer from "./components/Footer";
import Dashboard from "./pages/Dashboard";

render(
  () => (
    <>
      <Dashboard />
      <Footer />
    </>
  ),
  document.getElementById("root")!,
);
