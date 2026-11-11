import { Section, ValueRow } from "../sceneEditor/Form";
import { Lines } from "./StreamSettings";
import type { StreamFormProps } from "./StreamsSettings";
import { mobcamStreamPort } from "../../data/moblinStreamSettings";
import { urlPort } from "../../data/streamUrls";
import { t } from "../../i18n";

export function MobcamSettings(props: StreamFormProps) {
  const port = () => {
    const port = urlPort(props.stream.url);
    return port === null || port > 65535 ? mobcamStreamPort : port;
  };
  return (
    <Section
      footer={
        <Lines
          lines={[
            t(
              "Connect this device to a computer with a USB cable and run the Moblin Mobcam host tool on the computer to receive the stream. The computer connects to the port above over the cable.",
            ),
            "",
            t("Change the port by editing the URL."),
          ]}
        />
      }
    >
      <ValueRow label={t("Port")} value={String(port())} />
    </Section>
  );
}
