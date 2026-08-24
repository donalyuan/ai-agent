import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";
import type { AssetVersionRef, ReviewSession } from "./contracts";

export type ReviewPresentationSlice = {
  activeSessionId: string | null;
  sessionRevision: number | null;
  targetId: string | null;
  primary: AssetVersionRef | null;
  references: AssetVersionRef[];
};

const emptySlice = (): ReviewPresentationSlice => ({
  activeSessionId: null,
  sessionRevision: null,
  targetId: null,
  primary: null,
  references: [],
});

export const EMPTY_REVIEW_SLICE = Object.freeze(emptySlice());

export const reviewSliceKey = (projectId: string, episodeId: string) =>
  `${projectId}::${episodeId}`;

const sameVersion = (left: AssetVersionRef, right: AssetVersionRef) =>
  left.assetVersionId === right.assetVersionId &&
  left.revision === right.revision &&
  left.contentHash === right.contentHash &&
  left.kind === right.kind &&
  left.projectId === right.projectId;

export function validateReviewSlice(
  slice: ReviewPresentationSlice,
  owner: Pick<ReviewSession, "projectId" | "episodeId" | "targetId"> & {
    id?: string;
    revision?: number;
    sessionId?: string;
    sessionRevision?: number;
    primary: AssetVersionRef;
    references: AssetVersionRef[];
  },
): { slice: ReviewPresentationSlice; diagnostics: string[] } {
  const ownerSessionId = owner.sessionId ?? owner.id ?? "";
  const ownerRevision = owner.sessionRevision ?? owner.revision ?? -1;
  const invalid =
    slice.activeSessionId !== ownerSessionId ||
    slice.sessionRevision !== ownerRevision ||
    slice.targetId !== owner.targetId ||
    !slice.primary ||
    !sameVersion(slice.primary, owner.primary) ||
    slice.references.length !== owner.references.length ||
    slice.references.some(
      (item, index) =>
        !owner.references[index] || !sameVersion(item, owner.references[index]),
    );
  if (!invalid) return { slice, diagnostics: [] };
  const diagnostics = [
    slice.activeSessionId === ownerSessionId &&
    slice.sessionRevision !== ownerRevision
      ? "active_session_revision_stale"
      : "active_session_scope_invalid",
  ];
  return { slice: emptySlice(), diagnostics };
}

type Store = {
  slices: Record<string, ReviewPresentationSlice>;
  diagnostics: Record<string, string[]>;
  getSlice: (projectId: string, episodeId: string) => ReviewPresentationSlice;
  patchSlice: (
    projectId: string,
    episodeId: string,
    patch: Partial<ReviewPresentationSlice>,
  ) => void;
  switchScope: (projectId: string, episodeId: string, targetId: string) => void;
  restoreOwnerSession: (session: ReviewSession) => ReviewPresentationSlice;
  clearSlice: (
    projectId: string,
    episodeId: string,
    diagnostic?: string,
  ) => void;
};

export const useAssetEditReviewStore = create<Store>()(
  persist(
    (set, get) => ({
      slices: {},
      diagnostics: {},
      getSlice(projectId, episodeId) {
        return (
          get().slices[reviewSliceKey(projectId, episodeId)] ?? emptySlice()
        );
      },
      patchSlice(projectId, episodeId, patch) {
        const key = reviewSliceKey(projectId, episodeId);
        const current = get().slices[key] ?? emptySlice();
        set({ slices: { ...get().slices, [key]: { ...current, ...patch } } });
      },
      switchScope(projectId, episodeId, targetId) {
        const key = reviewSliceKey(projectId, episodeId);
        const current = get().slices[key] ?? emptySlice();
        if (current.targetId === targetId) return;
        set({
          slices: { ...get().slices, [key]: { ...emptySlice(), targetId } },
        });
      },
      restoreOwnerSession(session) {
        const key = reviewSliceKey(session.projectId, session.episodeId);
        const current = get().slices[key] ?? emptySlice();
        const result = validateReviewSlice(current, {
          ...session,
          primary: session.selection.primary,
          references: session.selection.references,
        });
        const restored =
          result.diagnostics.length === 0
            ? result.slice
            : {
                activeSessionId: session.id,
                sessionRevision: session.revision,
                targetId: session.targetId,
                primary: session.selection.primary,
                references: session.selection.references,
              };
        set({
          slices: { ...get().slices, [key]: restored },
          diagnostics: { ...get().diagnostics, [key]: result.diagnostics },
        });
        return restored;
      },
      clearSlice(projectId, episodeId, diagnostic) {
        const key = reviewSliceKey(projectId, episodeId);
        set({
          slices: { ...get().slices, [key]: emptySlice() },
          diagnostics: {
            ...get().diagnostics,
            [key]: diagnostic ? [diagnostic] : [],
          },
        });
      },
    }),
    {
      name: "video-agent-asset-edit-review-v1",
      storage: createJSONStorage(() => localStorage),
      partialize: (state) => ({ slices: state.slices }),
    },
  ),
);
