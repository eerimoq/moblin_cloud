import { asset } from "../asset";
import Footer from "../components/Footer";
import Mascot from "../components/Mascot";
import { buttonClass, CloudHeader } from "../components/PageParts";
import { pages } from "../data/links";
import { t } from "../i18n";

export default function NotFound() {
  return (
    <>
      <CloudHeader />
      <main>
        <section class="mx-auto flex max-w-[1120px] flex-col items-center gap-6 px-5 py-20 text-center sm:px-8 lg:py-28">
          <Mascot variant="looking" class="w-[150px] -rotate-[6deg]" />
          <h1 class="text-[40px] leading-[1.05] font-bold text-balance sm:text-6xl">
            {t("Page not found.")}
          </h1>
          <p class="text-lg text-ink-2 text-pretty sm:text-[21px]">
            {t("There is nothing here. It may have moved.")}
          </p>
          <a href={asset(pages.dashboard)} class={buttonClass}>
            {t("Open your dashboard")}
          </a>
        </section>
      </main>
      <Footer />
    </>
  );
}
