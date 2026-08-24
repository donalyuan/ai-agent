import { create } from "zustand";
import { emptyAssetFilters, type FilterState } from "./contracts";

type AssetCenterState = {
  projectId: string;
  selectedAssetId: string | null;
  filters: FilterState;
  playing: boolean;
  enterProject: (projectId: string) => void;
  selectAsset: (assetId: string | null) => void;
  setFilter: (name: keyof FilterState, value: string) => void;
  resetFilters: () => void;
  setPlaying: (playing: boolean) => void;
};

/** Owner DTO/session/media data never enters this non-persisted interaction store. */
export const useAssetCenterStore = create<AssetCenterState>((set) => ({
  projectId: "",
  selectedAssetId: null,
  filters: { ...emptyAssetFilters },
  playing: false,
  enterProject: (projectId) =>
    set((state) =>
      state.projectId === projectId
        ? state
        : {
            projectId,
            selectedAssetId: null,
            filters: { ...emptyAssetFilters },
            playing: false,
          },
    ),
  selectAsset: (selectedAssetId) => set({ selectedAssetId, playing: false }),
  setFilter: (name, value) =>
    set((state) => ({ filters: { ...state.filters, [name]: value } })),
  resetFilters: () => set({ filters: { ...emptyAssetFilters } }),
  setPlaying: (playing) => set({ playing }),
}));
