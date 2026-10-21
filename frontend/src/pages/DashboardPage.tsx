import { asset } from "../asset";
import Footer from "../components/Footer";
import { pages } from "../data/links";
import { t, tRich } from "../i18n";
import Dashboard from "./Dashboard";

export default function DashboardPage() {
  return (
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
  );
}
