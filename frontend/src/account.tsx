/* @refresh reload */
import { render } from "solid-js/web";
import "./index.css";
import Footer from "./components/Footer";
import AccountPage from "./pages/Account";

render(
  () => (
    <>
      <AccountPage />
      <Footer />
    </>
  ),
  document.getElementById("root")!,
);
