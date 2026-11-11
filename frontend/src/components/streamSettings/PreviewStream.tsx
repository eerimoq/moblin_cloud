import { MenuRow, PlainRow, Secret, Section, TextRow } from "../sceneEditor/Form";
import { formatBytesPerSecond } from "./Recording";
import type { StreamFormProps } from "./StreamsSettings";
import { streamResolutions, type StreamResolution } from "../../data/moblinStreamSettings";
import { cleanUrl, isValidUrl } from "../../data/streamUrls";
import { t } from "../../i18n";

const resolutions: StreamResolution[] = ["854x480", "640x360", "426x240"];

const videoBitrates = [2_000_000, 1_500_000, 1_000_000, 500_000, 250_000];

export function PreviewStreamSettings(props: StreamFormProps) {
  const previewStream = () => props.stream.previewStream;
  return (
    <>
      <Section>
        <PlainRow>
          {t(
            "A low-quality low-latency stream sent to a WHIP server. Can be used to preview the stream from another device.",
          )}
        </PlainRow>
      </Section>
      <Section>
        <Secret allowed={props.remote.secrets} label={t("URL")}>
          <TextRow
            stack={props.stack}
            label={t("URL")}
            value={previewStream().url}
            placeholder="whip://your-server/live"
            multiline
            masked
            validate={(value) => isValidUrl(cleanUrl(value), ["whip", "whips"])}
            onSubmit={(value) =>
              props.edit((draft) => {
                draft.previewStream.url = cleanUrl(value);
              })
            }
          />
        </Secret>
        <MenuRow
          label={t("Resolution")}
          items={streamResolutions.filter((resolution) => resolutions.includes(resolution.id))}
          selected={previewStream().resolution}
          onPick={(id) =>
            props.edit((draft) => {
              draft.previewStream.resolution = id as StreamResolution;
            })
          }
        />
        <MenuRow
          label={t("Video bitrate")}
          items={videoBitrates.map((bitrate) => ({
            id: String(bitrate),
            name: formatBytesPerSecond(bitrate),
          }))}
          selected={String(previewStream().bitrate)}
          onPick={(id) =>
            props.edit((draft) => {
              draft.previewStream.bitrate = Number(id);
            })
          }
        />
      </Section>
    </>
  );
}
