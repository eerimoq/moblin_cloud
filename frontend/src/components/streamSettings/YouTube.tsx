import { PlainRow, Section, TextRow } from "../sceneEditor/Form";
import type { StreamFormProps } from "./StreamsSettings";
import { t } from "../../i18n";

export function YouTubeSettings(props: StreamFormProps) {
  return (
    <>
      <Section footer={t("Log in and out on the phone.")}>
        <PlainRow>
          {props.stream.youTubeWantsToBeLoggedIn ? t("Logged in") : t("Not logged in")}
        </PlainRow>
      </Section>
      <Section footer={t("The Video ID unique for every live stream.")}>
        <TextRow
          stack={props.stack}
          label={t("Channel handle")}
          value={props.stream.youTubeHandle}
          placeholder="@erimo144"
          onSubmit={(value) =>
            props.edit((draft) => {
              draft.youTubeHandle = value;
            })
          }
        />
        <TextRow
          stack={props.stack}
          label={t("Video IDs")}
          value={props.stream.youTubeVideoId}
          placeholder="FekKCUN5W8U"
          onSubmit={(value) =>
            props.edit((draft) => {
              draft.youTubeVideoId = value.replace(/\s/g, "");
            })
          }
        />
      </Section>
    </>
  );
}
