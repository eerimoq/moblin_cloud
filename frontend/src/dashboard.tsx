/* @refresh reload */
import { render } from "solid-js/web";
import "./index.css";
import { asset } from "./asset";
import Footer from "./components/Footer";
import { pages } from "./data/links";
import { t, tRich } from "./i18n";
import Dashboard from "./pages/Dashboard";

render(
  () => (
    <>
      <Dashboard />
      <Footer
        note={tRich("Using Moblin Cloud and your tip page means you agree to the {terms}.", {
          terms: (
            <a href={asset(pages.terms)} class="text-leaf hover:text-leaf-bright">
              {t("terms")}
            </a>
          ),
        })}
      />
    </>
  ),
  document.getElementById("root")!,
);
