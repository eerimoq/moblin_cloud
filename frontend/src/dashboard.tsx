/* @refresh reload */
import { render } from "solid-js/web";
import "./index.css";
import Footer from "./components/Footer";
import Nav from "./components/Nav";
import Dashboard from "./pages/Dashboard";

render(
  () => (
    <>
      <Nav />
      <main>
        <Dashboard />
      </main>
      <Footer />
    </>
  ),
  document.getElementById("root")!,
);
