import { For, type JSX } from "solid-js";
import { asset } from "../asset";
import Footer from "../components/Footer";
import Nav from "../components/Nav";
import { Band, cardClass, Hero } from "../components/PageParts";
import { links, pages } from "../data/links";

const linkClass = "text-leaf hover:text-leaf-bright";

const sections: { title: string; body: JSX.Element[] }[] = [
  {
    title: "Moblin is not the seller",
    body: [
      <>
        Streamer pages on moblin.app let viewers request songs, send tips and buy wheel spins. When
        you pay for one of these, you pay the streamer directly through PayPal. The streamer is the
        seller and the payment goes to the streamer's PayPal account.
      </>,
      <>
        Moblin only provides the software. Moblin does not receive, hold or forward the money and
        takes no fee. PayPal charges the streamer its usual fees.
      </>,
    ],
  },
  {
    title: "Viewers",
    body: [
      <>
        A paid song request is added to the streamer's song queue in Moblin. The streamer decides
        what plays and when, and may skip or remove any song. A wheel spin is carried out by the
        streamer, the way they describe it on their stream.
      </>,
      <>
        Tips are voluntary gifts to the streamer. They are not charitable donations and you get
        nothing in return.
      </>,
      <>
        Questions about a payment, refunds and disputes are between you and the streamer. Contact
        the streamer first, or use PayPal's resolution center. Moblin cannot refund payments.
      </>,
    ],
  },
  {
    title: "Streamers",
    body: [
      <>
        When you connect PayPal, you sign up or sign in on PayPal and give Moblin permission to
        create payments to your PayPal account. Moblin cannot move money out of your account or
        refund payments. You can remove the permission in your PayPal settings at any time.
      </>,
      <>
        You are responsible for what you offer on your page and for delivering it, for refunds and
        disputes with your viewers, for any taxes on what you receive, and for following PayPal's
        terms and the rules of the platforms you stream on.
      </>,
      <>
        If you set a webhook URL, tips are sent to it as they come in. Moblin tries a few times and
        gives up if your server does not answer, so don't rely on it as your only record; PayPal has
        all your payments.
      </>,
    ],
  },
  {
    title: "Fair use",
    body: [
      <>
        Don't use Moblin for fraud, for anything illegal or to take payments for anything other than
        what your page shows. Moblin may remove pages and disconnect accounts that do.
      </>,
    ],
  },
  {
    title: "No warranty",
    body: [
      <>
        Moblin is free and provided as it is. It may be unavailable, have bugs or change, and
        features may be removed. As far as the law allows, Moblin is not liable for lost payments,
        songs that don't play, missed tips or spins, or any other loss from using it.
      </>,
    ],
  },
  {
    title: "Changes and contact",
    body: [
      <>
        These terms may change. The date at the top shows when they last did. How Moblin handles
        data is described in the{" "}
        <a href={links.privacyPolicy} target="_blank" rel="noopener" class={linkClass}>
          privacy policy
        </a>
        . Questions are welcome on{" "}
        <a href={links.discord} target="_blank" rel="noopener" class={linkClass}>
          Discord
        </a>
        .
      </>,
    ],
  },
];

export default function Terms() {
  return (
    <>
      <Nav />
      <main>
        <Hero eyebrow="Terms" title="Terms for streamer pages">
          <p class="max-w-[640px] text-lg text-ink-2 text-pretty sm:text-[21px]">
            For everyone who uses a streamer page on moblin.app or the{" "}
            <a href={asset(pages.dashboard)} class={linkClass}>
              dashboard
            </a>
            , and for the songs, tips and wheel spins viewers pay streamers for there.
          </p>
          <p class="text-[15px] text-muted">Last updated October 16, 2026.</p>
        </Hero>
        <Band>
          <For each={sections}>
            {(section) => (
              <div class={cardClass}>
                <h2 class="text-[23px] font-semibold">{section.title}</h2>
                <For each={section.body}>
                  {(paragraph) => <p class="max-w-[720px] text-ink-2 text-pretty">{paragraph}</p>}
                </For>
              </div>
            )}
          </For>
        </Band>
      </main>
      <Footer />
    </>
  );
}
