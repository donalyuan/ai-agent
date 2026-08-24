import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";

export type EpisodeFilters = {
  status: string;
  model: string;
  review: string;
};

export type EpisodePresentationSlice = {
  viewport: "storyboard" | "workflow" | "asset-bible";
  collapsedSceneIds: string[];
  filters: EpisodeFilters;
  selectedShotId: string | null;
  selectedAssetId: string | null;
  activeSessionId: string | null;
};

export type EpisodeOwnerScope = {
  projectId: string;
  episodeId: string;
  shotIds: ReadonlySet<string>;
  assetIds: ReadonlySet<string>;
  sessionIds: ReadonlySet<string>;
};

const initialSlice = (): EpisodePresentationSlice => ({
  viewport: "storyboard",
  collapsedSceneIds: [],
  filters: { status: "all", model: "all", review: "all" },
  selectedShotId: null,
  selectedAssetId: null,
  activeSessionId: null,
});

export const EMPTY_EPISODE_SLICE = Object.freeze(initialSlice());

export const episodeSliceKey = (projectId: string, episodeId: string) =>
  `${projectId}::${episodeId}`;

export function validateEpisodeSlice(
  slice: EpisodePresentationSlice,
  scope: EpisodeOwnerScope,
): { slice: EpisodePresentationSlice; diagnostics: string[] } {
  const next = { ...slice };
  const diagnostics: string[] = [];
  if (next.selectedShotId && !scope.shotIds.has(next.selectedShotId)) {
    next.selectedShotId = null;
    diagnostics.push("selected_shot_scope_invalid");
  }
  if (next.selectedAssetId && !scope.assetIds.has(next.selectedAssetId)) {
    next.selectedAssetId = null;
    diagnostics.push("selected_asset_scope_invalid");
  }
  if (next.activeSessionId && !scope.sessionIds.has(next.activeSessionId)) {
    next.activeSessionId = null;
    diagnostics.push("active_session_scope_invalid");
  }
  return { slice: next, diagnostics };
}

type PresentationState = {
  slices: Record<string, EpisodePresentationSlice>;
  diagnostics: Record<string, string[]>;
  getSlice: (projectId: string, episodeId: string) => EpisodePresentationSlice;
  patchSlice: (
    projectId: string,
    episodeId: string,
    patch: Partial<EpisodePresentationSlice>,
  ) => void;
  restoreSlice: (scope: EpisodeOwnerScope) => EpisodePresentationSlice;
  clearProject: (projectId: string) => void;
};

export const usePresentationStore = create<PresentationState>()(
  persist(
    (set, get) => ({
      slices: {},
      diagnostics: {},
      getSlice(projectId, episodeId) {
        return (
          get().slices[episodeSliceKey(projectId, episodeId)] ?? initialSlice()
        );
      },
      patchSlice(projectId, episodeId, patch) {
        const key = episodeSliceKey(projectId, episodeId);
        const current = get().slices[key] ?? initialSlice();
        const allowed: Partial<EpisodePresentationSlice> = {};
        for (const field of [
          "viewport",
          "collapsedSceneIds",
          "filters",
          "selectedShotId",
          "selectedAssetId",
          "activeSessionId",
        ] as const) {
          if (field in patch) Object.assign(allowed, { [field]: patch[field] });
        }
        set({ slices: { ...get().slices, [key]: { ...current, ...allowed } } });
      },
      restoreSlice(scope) {
        const key = episodeSliceKey(scope.projectId, scope.episodeId);
        const result = validateEpisodeSlice(
          get().slices[key] ?? initialSlice(),
          scope,
        );
        set({
          slices: { ...get().slices, [key]: result.slice },
          diagnostics: { ...get().diagnostics, [key]: result.diagnostics },
        });
        return result.slice;
      },
      clearProject(projectId) {
        set({
          slices: Object.fromEntries(
            Object.entries(get().slices).filter(
              ([key]) => !key.startsWith(`${projectId}::`),
            ),
          ),
        });
      },
    }),
    {
      name: "video-agent-episode-presentation-v1",
      storage: createJSONStorage(() => localStorage),
      partialize: (state) => ({ slices: state.slices }),
    },
  ),
);
