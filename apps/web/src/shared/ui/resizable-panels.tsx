import {
  Panel,
  PanelGroup,
  PanelResizeHandle,
  type PanelGroupProps,
  type PanelProps,
} from "react-resizable-panels";
import { cn } from "../lib/utils";

function ResizablePanelGroup({ className, ...props }: PanelGroupProps) {
  return <PanelGroup className={cn("flex min-h-0", className)} {...props} />;
}

function ResizablePanel({ className, ...props }: PanelProps) {
  return <Panel className={cn("min-h-0 min-w-0", className)} {...props} />;
}

function ResizableHandle({ className }: { className?: string }) {
  return (
    <PanelResizeHandle
      className={cn(
        "group relative flex w-2 shrink-0 items-center justify-center bg-transparent outline-none before:absolute before:inset-y-0 before:left-1/2 before:w-px before:-translate-x-1/2 before:bg-border focus-visible:before:bg-ring",
        className,
      )}
    >
      <span className="h-8 w-1 rounded-full bg-border group-hover:bg-ring" />
    </PanelResizeHandle>
  );
}

export { ResizableHandle, ResizablePanel, ResizablePanelGroup };
