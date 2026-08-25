import { QueryClientProvider } from "@tanstack/react-query";
import { BrowserRouter, Route, Routes, useParams } from "react-router";
import { AssetCenterPage } from "./pages/AssetCenterPage";
import { ExportsPage } from "./pages/ExportsPage";
import { ProviderSettingsPage } from "./pages/ProviderSettingsPage";
import { ReviewPage } from "./pages/ReviewPage";
import { StorageProfilePage } from "./pages/StorageProfilePage";
import { ProjectsPage } from "./pages/ProjectsPage";
import { TimelineEditorPage } from "./pages/TimelineEditorPage";
import { TimelineSelectorPage } from "./pages/TimelineSelectorPage";
import { WorkbenchPage } from "./pages/WorkbenchPage";
import { PhaseOneWorkbenchPrototype } from "./prototype/PhaseOneWorkbenchPrototype";
import { ProjectShell } from "./layouts/ProjectShell";
import { queryClient } from "./app/query-client";

export { queryClient } from "./app/query-client";

function ProjectSettingsRoute() {
  const { projectId = "" } = useParams();
  return <ProviderSettingsPage projectId={projectId} />;
}

function Shell() {
  return (
    <ProjectShell>
      <Routes>
        <Route path="/projects" element={<ProjectsPage />} />
        <Route
          path="/projects/:projectId/workbench"
          element={<WorkbenchPage />}
        />
        <Route path="/projects/:projectId/review" element={<ReviewPage />} />
        <Route
          path="/projects/:projectId/assets"
          element={<AssetCenterPage />}
        />
        <Route
          path="/projects/:projectId/episodes/:episodeId/timeline"
          element={<TimelineEditorPage />}
        />
        <Route
          path="/projects/:projectId/episodes/select/timeline"
          element={<TimelineSelectorPage />}
        />
        <Route path="/projects/:projectId/exports" element={<ExportsPage />} />
        <Route
          path="/projects/:projectId/settings"
          element={<ProjectSettingsRoute />}
        />
        <Route
          path="/settings/providers"
          element={<ProviderSettingsPage projectId="" />}
        />
        <Route
          path="/settings/providers/:providerId"
          element={<ProviderSettingsPage projectId="" />}
        />
        <Route
          path="/settings/skills"
          element={<ProviderSettingsPage projectId="" />}
        />
        <Route
          path="/projects/:projectId/settings/storage-profiles/:storageProfileId"
          element={<StorageProfilePage />}
        />
        <Route path="*" element={<ProjectsPage />} />
      </Routes>
    </ProjectShell>
  );
}

export function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/prototype" element={<PhaseOneWorkbenchPrototype />} />
        <Route
          path="*"
          element={
            <QueryClientProvider client={queryClient}>
              <Shell />
            </QueryClientProvider>
          }
        />
      </Routes>
    </BrowserRouter>
  );
}
