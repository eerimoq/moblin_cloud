import { PlainRow, Secret, Section, TextRow } from "../sceneEditor/Form";
import type { StreamFormProps } from "./StreamsSettings";
import { isValidHttpUrl } from "../../data/streamUrls";
import { t } from "../../i18n";

export function RealtimeIrlSettings(props: StreamFormProps) {
  return (
    <>
      <Section>
        <PlainRow>
          {t("Send your location to https://rtirl.com, to let your viewers know where you are.")}
        </PlainRow>
      </Section>
      <Section>
        <Secret allowed={props.remote.secrets} label={t("Base URL")}>
          <TextRow
            stack={props.stack}
            label={t("Base URL")}
            value={props.stream.realtimeIrlBaseUrl}
            placeholder="https://rtirl.com/api"
            validate={isValidHttpUrl}
            onSubmit={(value) =>
              props.edit((draft) => {
                draft.realtimeIrlBaseUrl = value;
              })
            }
          />
        </Secret>
        <Secret allowed={props.remote.secrets} label={t("Push key")}>
          <TextRow
            stack={props.stack}
            label={t("Push key")}
            value={props.stream.realtimeIrlPushKey}
            masked
            onSubmit={(value) =>
              props.edit((draft) => {
                draft.realtimeIrlPushKey = value;
              })
            }
          />
        </Secret>
      </Section>
    </>
  );
}
