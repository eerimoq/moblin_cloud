import { Section, TextRow } from "../sceneEditor/Form";
import type { StreamFormProps } from "./StreamsSettings";
import { t } from "../../i18n";

export function SoopSettings(props: StreamFormProps) {
  return (
    <Section
      footer={
        <span class="flex flex-col">
          <span>{t("Very experimental and very secret!")}</span>
          <span>&nbsp;</span>
          <span>
            {t(
              "Find your channel name (myChannelName) and video id (myVideoId) in your stream's URL.",
            )}
          </span>
          <span>{t("Example URL: https://play.sooplive.co.kr/myChannelName/myVideoId")}</span>
        </span>
      }
    >
      <TextRow
        stack={props.stack}
        label={t("Channel name")}
        value={props.stream.afreecaTvChannelName}
        onSubmit={(value) =>
          props.edit((draft) => {
            draft.afreecaTvChannelName = value;
          })
        }
      />
      <TextRow
        stack={props.stack}
        label={t("Video id")}
        value={props.stream.afreecaTvStreamId}
        onSubmit={(value) =>
          props.edit((draft) => {
            draft.afreecaTvStreamId = value;
          })
        }
      />
    </Section>
  );
}
