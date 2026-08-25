import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import {
  SortableClipLane,
  reorderClipIds,
  type Clip,
} from "./sortable-clip-lane";

const clip = (id: string, parentId = "root"): Clip => ({
  id,
  assetVersionId: `asset-${id}`,
  assetVersionRevision: 0,
  assetVersionHash: "a".repeat(64),
  timelineStart: 0,
  durationFrames: 30,
  inFrame: 0,
  outFrame: 30,
  parentId,
});

describe("SortableClipLane", () => {
  it("only reorders within one parent scope and keeps a complete id set", () => {
    const clips = [clip("a"), clip("b"), clip("c", "nested")];
    expect(reorderClipIds(clips, "a", "b")).toEqual(["b", "a", "c"]);
    expect(reorderClipIds(clips, "a", "c")).toBeNull();
  });

  it("exposes keyboard sortable semantics and emits a reorder command candidate", () => {
    const onReorder = vi.fn();
    render(
      <SortableClipLane clips={[clip("a"), clip("b")]} onReorder={onReorder} />,
    );
    const first = screen.getByRole("listitem", { name: "Clip a" });
    expect(first).toHaveAttribute("aria-roledescription", "sortable clip");
    expect(first).toHaveAttribute("tabindex", "0");
    fireEvent.keyDown(first, { key: "ArrowRight" });
    expect(onReorder).toHaveBeenCalledWith(["b", "a"]);
  });
});
