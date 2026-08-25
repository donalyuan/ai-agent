import { useVirtualizer } from "@tanstack/react-virtual";
import * as React from "react";
import { cn } from "../lib/utils";

type VirtualListProps<T> = {
  items: T[];
  estimateSize?: number;
  height?: number;
  getKey: (item: T, index: number) => string;
  renderItem: (item: T, index: number) => React.ReactNode;
  className?: string;
  ariaLabel: string;
};

function VirtualList<T>({
  items,
  estimateSize = 44,
  height = 280,
  getKey,
  renderItem,
  className,
  ariaLabel,
}: VirtualListProps<T>) {
  const parentRef = React.useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => estimateSize,
    overscan: 6,
  });

  return (
    <div
      ref={parentRef}
      aria-label={ariaLabel}
      className={cn("overflow-auto rounded-md border border-border", className)}
      role="list"
      style={{ height }}
    >
      <div
        className="relative w-full"
        style={{ height: virtualizer.getTotalSize() }}
      >
        {virtualizer.getVirtualItems().map((virtualItem) => {
          const item = items[virtualItem.index];
          return (
            <div
              className="absolute top-0 left-0 w-full"
              data-index={virtualItem.index}
              key={getKey(item, virtualItem.index)}
              ref={virtualizer.measureElement}
              role="listitem"
              style={{ transform: `translateY(${virtualItem.start}px)` }}
            >
              {renderItem(item, virtualItem.index)}
            </div>
          );
        })}
      </div>
    </div>
  );
}

export { VirtualList };
export type { VirtualListProps };
