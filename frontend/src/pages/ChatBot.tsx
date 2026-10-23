import { For } from "solid-js";
import CopyButton from "../components/CopyButton";
import Footer from "../components/Footer";
import Mascot from "../components/Mascot";
import Nav from "../components/Nav";
import { commandGroups } from "../data/chatBot";
import { links } from "../data/links";
import { t, tRich } from "../i18n";

/** A command exactly as typed in chat, with a button that copies it to the clipboard. */
function Command(props: { text: string }) {
  return (
    <span class="flex items-start gap-2">
      <code class="font-mono text-[15px] font-semibold text-ink">{props.text}</code>
      <CopyButton text={props.text} />
    </span>
  );
}

export default function ChatBot() {
  return (
    <>
      <Nav />
      <main>
        <section class="mx-auto flex max-w-[1120px] flex-col gap-6 px-5 pt-12 pb-10 sm:px-8 lg:pt-18">
          <div class="flex items-start justify-between gap-6">
            <div class="flex max-w-[640px] flex-col gap-4">
              <span class="font-display text-[15px] font-semibold uppercase tracking-[0.08em] text-leaf">
                {t("Chat bot")}
              </span>
              <h1 class="text-[40px] leading-[1.05] font-bold text-balance sm:text-6xl">
                {t("Let chat run the show.")}
              </h1>
              <p class="text-lg text-ink-2 text-pretty sm:text-[21px]">
                {t(
                  "Viewers type commands in Twitch, YouTube or Kick chat and Moblin does the rest. " +
                    "Turn the bot on under Settings, Chat, Bot, and pick who may use each command: " +
                    "moderators, subscribers or everyone.",
                )}
              </p>
              <p class="text-[15px] text-muted">
                {tRich(
                  "Replace placeholders like {placeholder} with your own text. Parts in square " +
                    "brackets are optional.",
                  { placeholder: <code class="text-ink-2">&lt;name&gt;</code> },
                )}
              </p>
            </div>
            <Mascot
              variant="looking"
              class="hidden w-[150px] shrink-0 -rotate-[6deg] sm:block lg:w-[190px]"
            />
          </div>
        </section>

        <section class="bg-band py-16 lg:py-20">
          <div class="mx-auto flex max-w-[1120px] flex-col gap-6 px-5 sm:px-8">
            <For each={commandGroups}>
              {(group) => (
                <div class="flex flex-col gap-4 rounded-3xl border-[1.5px] border-line bg-card p-6 sm:p-8">
                  <h2 class="text-[23px] font-semibold">{group.title}</h2>
                  <table class="w-full border-collapse text-base">
                    <thead class="sr-only">
                      <tr>
                        <th scope="col">{t("Command")}</th>
                        <th scope="col">{t("Description")}</th>
                      </tr>
                    </thead>
                    <tbody>
                      <For each={group.commands}>
                        {(c) => (
                          <tr class="block border-t border-line py-3 first:border-t-0 first:pt-0 last:pb-0 sm:table-row sm:py-0 sm:[&:first-child>td]:pt-0 sm:[&:last-child>td]:pb-0">
                            <td class="block pb-1 align-top break-words sm:table-cell sm:w-1/2 sm:py-3 sm:pr-6">
                              <Command text={c.command} />
                            </td>
                            <td class="block align-top text-ink-2 sm:table-cell sm:py-3">
                              {c.text}
                            </td>
                          </tr>
                        )}
                      </For>
                    </tbody>
                  </table>
                </div>
              )}
            </For>
          </div>
        </section>

        <section class="mx-auto max-w-[1120px] px-5 py-12 sm:px-8">
          <p class="text-[15px] text-muted">
            {tRich("Kept in sync with the {list}. Something missing? Ask on {discord}.", {
              list: (
                <a
                  href={links.chatBotHelp}
                  target="_blank"
                  rel="noopener"
                  class="text-leaf hover:text-leaf-bright"
                >
                  {t("command list on GitHub")}
                </a>
              ),
              discord: (
                <a
                  href={links.discord}
                  target="_blank"
                  rel="noopener"
                  class="text-leaf hover:text-leaf-bright"
                >
                  Discord
                </a>
              ),
            })}
          </p>
        </section>
      </main>
      <Footer />
    </>
  );
}
