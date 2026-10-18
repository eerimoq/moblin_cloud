/* @refresh reload */
import { render } from "solid-js/web";
import "./index.css";
import Footer from "./components/Footer";
import Admin from "./pages/Admin";

render(
  () => (
    <>
      <Admin />
      <Footer />
    </>
  ),
  document.getElementById("root")!,
);
