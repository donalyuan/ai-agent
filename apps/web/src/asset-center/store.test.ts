import { beforeEach, describe, expect, it } from "vitest";
import { emptyAssetFilters } from "./contracts";
import { useAssetCenterStore } from "./store";

describe("asset center interaction store", () => {
  beforeEach(() => {
    useAssetCenterStore.setState({
      projectId: "",
      selectedAssetId: null,
      filters: { ...emptyAssetFilters },
      playing: false,
    });
  });

  it("only keeps presentation references and resets them across projects", () => {
    const store = useAssetCenterStore.getState();
    store.enterProject("project-a");
    store.selectAsset("asset-a");
    store.setFilter("kind", "audio");
    store.setPlaying(true);

    expect(useAssetCenterStore.getState()).toMatchObject({
      projectId: "project-a",
      selectedAssetId: "asset-a",
      filters: { kind: "audio" },
      playing: true,
    });
    expect(JSON.stringify(useAssetCenterStore.getState())).not.toMatch(
      /session|objectKey|accessPath|credential|bytes/i,
    );

    useAssetCenterStore.getState().enterProject("project-b");
    expect(useAssetCenterStore.getState()).toMatchObject({
      projectId: "project-b",
      selectedAssetId: null,
      filters: emptyAssetFilters,
      playing: false,
    });
  });
});
