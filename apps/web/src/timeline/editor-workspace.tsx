import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "../shared/ui";
import type { ReactNode } from "react";

export function TimelineEditorWorkspace({
  canvas,
  inspector,
}: {
  canvas: ReactNode;
  inspector: ReactNode;
}) {
  return (
    <ResizablePanelGroup
      direction="horizontal"
      className="h-[min(620px,70vh)] min-h-[420px] w-full overflow-hidden rounded-md border border-border bg-background"
      data-testid="timeline-panel-group"
    >
      <ResizablePanel
        defaultSize={64}
        minSize={50}
        maxSize={78}
        className="min-w-0 overflow-auto p-4"
      >
        {canvas}
      </ResizablePanel>
      <ResizableHandle />
      <ResizablePanel
        defaultSize={36}
        minSize={22}
        maxSize={50}
        className="min-w-0 overflow-auto border-l border-border p-4"
      >
        {inspector}
      </ResizablePanel>
    </ResizablePanelGroup>
  );
}
