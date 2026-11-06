import Footer from "../components/Footer";
import { TermsLink } from "../components/PageParts";
import { tRich } from "../i18n";
import { Show } from "solid-js";
import Dashboard, { fullWindow } from "./Dashboard";

export default function DashboardPage() {
  return (
    <>
      <Dashboard />
      <Show when={!fullWindow()}>
        <Footer
          note={
            window.config.tippingEnabled === false
              ? tRich("Using Moblin Cloud means you agree to the {terms}.", {
                  terms: <TermsLink leaf />,
                })
              : tRich("Using Moblin Cloud and your tip page means you agree to the {terms}.", {
                  terms: <TermsLink leaf />,
                })
          }
        />
      </Show>
    </>
  );
}
