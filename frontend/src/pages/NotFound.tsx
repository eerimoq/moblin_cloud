import { asset } from "../asset";
import Footer from "../components/Footer";
import Mascot from "../components/Mascot";
import Nav from "../components/Nav";
import { buttonClass } from "../components/PageParts";

export default function NotFound() {
  return (
    <>
      <Nav />
      <main>
        <section class="mx-auto flex max-w-[1120px] flex-col items-center gap-6 px-5 py-20 text-center sm:px-8 lg:py-28">
          <Mascot variant="looking" class="w-[150px] -rotate-[6deg]" />
          <h1 class="text-[40px] leading-[1.05] font-bold text-balance sm:text-6xl">
            Page not found.
          </h1>
          <p class="text-lg text-ink-2 text-pretty sm:text-[21px]">
            There is nothing here. It may have moved.
          </p>
          <a href={asset("/")} class={buttonClass}>
            Go to the home page
          </a>
        </section>
      </main>
      <Footer />
    </>
  );
}
