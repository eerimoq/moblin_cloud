import {
  noVideoSourceId,
  screenCaptureSourceId,
  type CameraPosition,
  type VideoSource,
  type VideoSourceFields,
} from "../../data/moblinSettings";
import { t } from "../../i18n";

const idField: Partial<Record<CameraPosition, keyof VideoSourceFields>> = {
  Back: "backCameraId",
  Front: "frontCameraId",
  RTMP: "rtmpCameraId",
  "SRT(LA)": "srtlaCameraId",
  "SRT client": "srtClientCameraId",
  RIST: "ristCameraId",
  RTSP: "rtspCameraId",
  WHIP: "whipCameraId",
  WHEP: "whepCameraId",
  "Media player": "mediaPlayerCameraId",
  External: "externalCameraId",
};

const fixedIds: Partial<Record<CameraPosition, string>> = {
  "Screen capture": screenCaptureSourceId,
  None: noVideoSourceId,
};

export const videoSourceId = (fields: VideoSourceFields): string => {
  const field = idField[fields.cameraPosition];
  if (field) {
    return String(fields[field]);
  }
  return fixedIds[fields.cameraPosition] ?? fields.cameraPosition;
};

export const applyVideoSource = (fields: VideoSourceFields, source: VideoSource) => {
  fields.cameraPosition = source.position;
  const field = idField[source.position];
  if (field) {
    (fields as unknown as Record<string, unknown>)[field] = source.id;
  }
  if (source.position === "External") {
    fields.externalCameraName = source.name;
  }
};

export const videoSourceName = (fields: VideoSourceFields, sources: VideoSource[]) => {
  const id = videoSourceId(fields).toUpperCase();
  const found = sources.find(
    (source) => source.position === fields.cameraPosition && source.id.toUpperCase() === id,
  );
  return found?.name ?? t("Unknown 😢");
};

export const videoSourceItems = (sources: VideoSource[]) =>
  sources.map((source) => ({ id: `${source.position}|${source.id}`, name: source.name }));

export const videoSourceItemId = (fields: VideoSourceFields) =>
  `${fields.cameraPosition}|${videoSourceId(fields)}`;

export const videoSourceMenuItems = (sources: VideoSource[], fields: VideoSourceFields) => {
  const items = videoSourceItems(sources);
  const selected = videoSourceItemId(fields);
  return items.some((item) => item.id === selected)
    ? items
    : [...items, { id: selected, name: t("Unknown 😢") }];
};

export const videoSourceOf = (sources: VideoSource[], itemId: string) =>
  sources.find((source) => `${source.position}|${source.id}` === itemId);
