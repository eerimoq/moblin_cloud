import { Section } from "../../sceneEditor/Form";
import { NextRow } from "../../sceneEditor/wizards/Sheet";
import { General } from "./General";
import { ExternalLink, pushPage, type WizardPageProps } from "./shared";
import { t, tRich } from "../../../i18n";

export function Mobcam(props: WizardPageProps) {
  const wizard = props.wizard;
  wizard.set({ platform: "mobcam", name: wizard.uniqueName(t("Custom Mobcam")) });
  return (
    <>
      <Section>
        <span class="px-4 py-2 text-[17px]">
          {t("Use Moblin as a low latency camera in OBS Studio over USB.")}
        </span>
      </Section>
      <Section header={t("Configure OBS on your computer")}>
        <div class="flex flex-col gap-4 px-4 py-2 text-[17px]">
          <span>
            {tRich("1. Install the OBS Mobcam Plugin as described {link}.", {
              link: (
                <ExternalLink href="https://github.com/eerimoq/mobcam/tree/main/crates/obs-plugin#mobcam-obs-plugin">
                  {t("here")}
                </ExternalLink>
              ),
            })}
          </span>
          <span>{t("2. Connect Moblin to the computer with a USB cable.")}</span>
          <span>{t("3. Add a Mobcam source in OBS.")}</span>
          <span>{t("4. Press Go live in Moblin to start the stream to OBS.")}</span>
        </div>
      </Section>
      <NextRow onClick={() => pushPage(wizard, t("General"), General)} />
    </>
  );
}
