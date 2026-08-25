import { useParams } from "react-router";
import { WorkbenchWorkspace } from "../workbench/WorkbenchWorkspace";

export function WorkbenchPage() {
  const { projectId = "" } = useParams();
  return <WorkbenchWorkspace projectId={projectId} />;
}
