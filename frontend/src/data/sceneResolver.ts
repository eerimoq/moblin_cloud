import {
  sameId,
  type SettingsScene,
  type SettingsSceneWidget,
  type SettingsWidget,
} from "./moblinSettings";

export type ResolvedSceneWidget = {
  scene: SettingsScene;
  sceneWidget: SettingsSceneWidget;
  widget: SettingsWidget;
  parentsEnabled: boolean;
  first: boolean;
};

export type ResolvedScene = { widgets: ResolvedSceneWidget[]; sceneIds: string[] };

export const resolvedEnabled = (resolved: ResolvedSceneWidget) =>
  resolved.parentsEnabled && resolved.widget.enabled;

export const firstWidgets = (resolved: ResolvedScene) =>
  resolved.widgets.filter((widget) => widget.first);

type Change =
  | { add: { sceneId: string; widgetId: string } }
  | { setScene: { widgetId: string; sceneId: string } };

const duplicatedWidgetCounts = (resolved: ResolvedScene) => {
  const counts = new Map<string, number>();
  for (const widget of resolved.widgets) {
    if (!widget.first) {
      const key = widget.widget.id.toUpperCase();
      counts.set(key, (counts.get(key) ?? 0) + 1);
    }
  }
  return counts;
};

export class SceneResolver {
  constructor(
    private readonly scenes: SettingsScene[],
    private readonly widgets: SettingsWidget[],
  ) {}

  findScene(id: string | null | undefined) {
    return this.scenes.find((scene) => sameId(scene.id, id));
  }

  findWidget(id: string | null | undefined) {
    return this.widgets.find((widget) => sameId(widget.id, id));
  }

  resolve(scene: SettingsScene, includeDisabledSceneWidgets = false): ResolvedScene {
    return this.walk(scene, includeDisabledSceneWidgets, undefined);
  }

  addableWidgets(scene: SettingsScene) {
    const before = this.resolveAll();
    return this.widgets.filter(
      (widget) =>
        !this.introducesDuplicates({ add: { sceneId: scene.id, widgetId: widget.id } }, before),
    );
  }

  selectableScenes(widget: SettingsWidget) {
    const before = this.resolveAll();
    return this.scenes.filter(
      (scene) =>
        !this.introducesDuplicates(
          { setScene: { widgetId: widget.id, sceneId: scene.id } },
          before,
        ),
    );
  }

  private resolveAll() {
    return this.scenes.map((scene) => this.walk(scene, true, undefined));
  }

  private introducesDuplicates(change: Change, before: ResolvedScene[]) {
    return this.scenes.some((scene, index) => {
      const resolved = before[index]!;
      if ("add" in change) {
        if (!resolved.sceneIds.some((id) => sameId(id, change.add.sceneId))) {
          return false;
        }
      } else if (
        !resolved.widgets.some((widget) => sameId(widget.widget.id, change.setScene.widgetId))
      ) {
        return false;
      }
      const beforeCounts = duplicatedWidgetCounts(resolved);
      const afterCounts = duplicatedWidgetCounts(this.walk(scene, true, change));
      return [...afterCounts].some(
        ([widgetId, count]) => count > (beforeCounts.get(widgetId) ?? 0),
      );
    });
  }

  private sceneWidgets(scene: SettingsScene, change: Change | undefined): SettingsSceneWidget[] {
    if (change && "add" in change && sameId(change.add.sceneId, scene.id)) {
      return [
        ...scene.widgets,
        {
          id: "added",
          widgetId: change.add.widgetId,
          x: 0,
          y: 0,
          size: 100,
          alignment: "TopLeft",
          positioningLock: false,
        },
      ];
    }
    return scene.widgets;
  }

  private walk(
    scene: SettingsScene,
    includeDisabledSceneWidgets: boolean,
    change: Change | undefined,
  ) {
    const resolved: ResolvedScene = { widgets: [], sceneIds: [scene.id] };
    const widgetIds = new Set<string>();
    const visit = (
      current: SettingsScene,
      sceneWidgets: SettingsSceneWidget[],
      parentsEnabled: boolean,
    ) => {
      for (const sceneWidget of sceneWidgets) {
        const widget = this.findWidget(sceneWidget.widgetId);
        if (!widget) {
          continue;
        }
        const key = widget.id.toUpperCase();
        const first = !widgetIds.has(key);
        widgetIds.add(key);
        resolved.widgets.push({ scene: current, sceneWidget, widget, parentsEnabled, first });
        if (widget.type !== "Scene" || !(widget.enabled || includeDisabledSceneWidgets)) {
          continue;
        }
        let nestedSceneId = widget.scene.sceneId;
        if (change && "setScene" in change && sameId(change.setScene.widgetId, widget.id)) {
          nestedSceneId = change.setScene.sceneId;
        }
        const nested = this.findScene(nestedSceneId);
        if (!nested || resolved.sceneIds.some((id) => sameId(id, nested.id))) {
          continue;
        }
        resolved.sceneIds.push(nested.id);
        visit(nested, this.sceneWidgets(nested, change), parentsEnabled && widget.enabled);
      }
    };
    visit(scene, this.sceneWidgets(scene, change), true);
    return resolved;
  }
}
