import {
  DndContext,
  KeyboardSensor,
  PointerSensor,
  closestCenter,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core";
import {
  SortableContext,
  arrayMove,
  horizontalListSortingStrategy,
  sortableKeyboardCoordinates,
  useSortable,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { GripVertical } from "lucide-react";
import type { KeyboardEvent } from "react";
import type { Timeline } from "./contracts";

type Clip = Timeline["clips"][number];

const parentOf = (clip: Clip) => {
  const value = clip as Clip & {
    parentClipId?: string | null;
    parentId?: string | null;
  };
  return String(value.parentClipId ?? value.parentId ?? "root");
};

export function reorderClipIds(
  clips: Clip[],
  activeId: string,
  overId: string,
): string[] | null {
  const active = clips.find((clip) => clip.id === activeId);
  const over = clips.find((clip) => clip.id === overId);
  if (!active || !over || parentOf(active) !== parentOf(over)) return null;

  const scopeIds = clips
    .filter((clip) => parentOf(clip) === parentOf(active))
    .map((clip) => clip.id);
  const from = scopeIds.indexOf(activeId);
  const to = scopeIds.indexOf(overId);
  if (from < 0 || to < 0 || from === to) return null;
  const moved = arrayMove(scopeIds, from, to);
  const next = [...clips.map((clip) => clip.id)];
  let cursor = 0;
  return next.map((id) => (scopeIds.includes(id) ? moved[cursor++] : id));
}

function SortableClip({
  clip,
  index,
  onKeyboardMove,
}: {
  clip: Clip;
  index: number;
  onKeyboardMove: (activeId: string, direction: -1 | 1) => void;
}) {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: clip.id });
  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.5 : 1,
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    event.preventDefault();
    onKeyboardMove(clip.id, event.key === "ArrowLeft" ? -1 : 1);
  };

  return (
    <div
      ref={setNodeRef}
      style={style}
      className="flex min-w-40 flex-1 basis-40 items-stretch rounded border border-border bg-card shadow-sm"
      data-clip-id={clip.id}
      data-parent-id={parentOf(clip)}
      {...attributes}
      role="listitem"
      tabIndex={0}
      aria-label={`Clip ${clip.id}`}
      aria-roledescription="sortable clip"
      onKeyDown={onKeyDown}
    >
      <button
        type="button"
        className="flex w-7 shrink-0 cursor-grab items-center justify-center border-r border-border text-muted-foreground hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        aria-label={`拖动 Clip ${clip.id}`}
        {...listeners}
      >
        <GripVertical className="size-4" />
      </button>
      <span className="grid min-w-0 gap-1 px-2 py-2 text-left text-xs">
        <strong className="truncate">{clip.assetVersionId}</strong>
        <span className="font-mono text-[10px] text-muted-foreground">
          {clip.timelineStart}f / {clip.durationFrames}f
        </span>
        <span className="sr-only">第 {index + 1} 个 Clip</span>
      </span>
    </div>
  );
}

export function SortableClipLane({
  clips,
  disabled = false,
  onReorder,
}: {
  clips: Clip[];
  disabled?: boolean;
  onReorder: (clipIds: string[]) => void;
}) {
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 4 } }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    }),
  );
  const groups = clips.reduce<Map<string, Clip[]>>((result, clip) => {
    const key = parentOf(clip);
    const group = result.get(key) ?? [];
    group.push(clip);
    result.set(key, group);
    return result;
  }, new Map());

  const finishDrag = (event: DragEndEvent) => {
    if (disabled || !event.over) return;
    const next = reorderClipIds(
      clips,
      String(event.active.id),
      String(event.over.id),
    );
    if (next) onReorder(next);
  };

  const moveWithKeyboard = (activeId: string, direction: -1 | 1) => {
    if (disabled) return;
    const active = clips.find((clip) => clip.id === activeId);
    if (!active) return;
    const group = groups.get(parentOf(active)) ?? [];
    const index = group.findIndex((clip) => clip.id === activeId);
    const over = group[index + direction];
    if (over) {
      const next = reorderClipIds(clips, activeId, over.id);
      if (next) onReorder(next);
    }
  };

  return (
    <div
      className="grid gap-2"
      role="list"
      aria-label="Timeline Clip lanes"
      data-testid="clip-lane"
    >
      {groups.size === 0 && (
        <p className="py-8 text-center text-sm text-muted-foreground">
          暂无 owner Clip
        </p>
      )}
      {Array.from(groups.entries()).map(([parentId, group]) => (
        <DndContext
          key={parentId}
          sensors={sensors}
          collisionDetection={closestCenter}
          onDragEnd={finishDrag}
        >
          <div className="grid gap-1" data-parent-scope={parentId}>
            <div className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
              {parentId === "root" ? "ROOT CLIP LANE" : `PARENT ${parentId}`}
            </div>
            <SortableContext
              items={group.map((clip) => clip.id)}
              strategy={horizontalListSortingStrategy}
            >
              <div className="flex min-h-20 gap-2 overflow-x-auto pb-1">
                {group.map((clip, index) => (
                  <SortableClip
                    key={clip.id}
                    clip={clip}
                    index={index}
                    onKeyboardMove={moveWithKeyboard}
                  />
                ))}
              </div>
            </SortableContext>
          </div>
        </DndContext>
      ))}
    </div>
  );
}

export type { Clip };
