import { For, type JSX } from "solid-js";
import { asset } from "../asset";
import Footer from "../components/Footer";
import { Band, cardClass, CloudHeader, Hero } from "../components/PageParts";
import { links, pages } from "../data/links";

const linkClass = "text-leaf hover:text-leaf-bright";

const sections: { title: string; body: JSX.Element[] }[] = [
  {
    title: "Tips go straight to the streamer",
    body: [
      <>
        Tip pages on cloud.moblin.app let viewers tip streamers. Every payment on a tip page is a
        tip, sent directly to the streamer's PayPal or Stripe account. Tips do not constitute the
        purchase of physical goods.
      </>,
      <>
        Moblin only provides the software. Moblin does not receive, hold or forward the money and
        takes no fee. PayPal and Stripe charge the streamer their usual fees.
      </>,
    ],
  },
  {
    title: "Viewers",
    body: [
      <>
        Tips are voluntary financial contributions to the streamer. They are not charitable
        donations and you are not owed anything in return.
      </>,
      <>
        A streamer may thank you for a tip, for example by spinning their wheel, but doesn't have
        to. The streamer decides what happens on their stream and when.
      </>,
      <>
        Questions about a tip, refunds and disputes are between you and the streamer. Contact the
        streamer first, or use PayPal's resolution center or your card issuer. Moblin cannot refund
        tips.
      </>,
    ],
  },
  {
    title: "Streamers",
    body: [
      <>
        When you connect PayPal, you sign up or sign in on PayPal and give Moblin permission to send
        tips to your PayPal account. Moblin cannot move money out of your account or refund tips.
        You can remove the permission in your PayPal settings at any time.
      </>,
      <>
        When you connect Stripe, you sign up or sign in on Stripe with an account that Moblin
        creates for you. Moblin only uses it to take tips for you, and never refunds tips or moves
        money out of it. You can disconnect Stripe in the dashboard at any time.
      </>,
      <>
        You are responsible for what your tip page says, for refunds and disputes with your viewers,
        for any taxes on what you receive, and for following PayPal's or Stripe's terms and the
        rules of the platforms you stream on.
      </>,
      <>
        Tips are sent to your Moblin app. When it is not connected, Moblin keeps them for 7 days and
        then gives up, so don't rely on it as your only record; PayPal and Stripe have all your
        tips.
      </>,
    ],
  },
  {
    title: "Fair use",
    body: [
      <>
        Don't use Moblin for fraud, for anything illegal or to sell anything; your tip page is for
        tips only. Moblin may remove tip pages and disconnect accounts that do.
      </>,
    ],
  },
  {
    title: "No warranty",
    body: [
      <>
        Moblin is free and provided as it is. It may be unavailable, have bugs or change, and
        features may be removed. As far as the law allows, Moblin is not liable for lost tips or any
        other loss from using it.
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
      <CloudHeader />
      <main>
        <Hero eyebrow="Terms" title="Terms for tip pages">
          <p class="max-w-[640px] text-lg text-ink-2 text-pretty sm:text-[21px]">
            For everyone who uses a tip page on cloud.moblin.app or the{" "}
            <a href={asset(pages.dashboard)} class={linkClass}>
              dashboard
            </a>
            , and for the tips viewers send streamers there.
          </p>
          <p class="text-[15px] text-muted">Last updated October 28, 2026.</p>
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
