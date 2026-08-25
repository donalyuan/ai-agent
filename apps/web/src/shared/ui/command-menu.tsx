import { Command } from "cmdk";
import { Search } from "lucide-react";
import { cn } from "../lib/utils";

type CommandItem = {
  id: string;
  label: string;
  keywords?: string;
  onSelect: () => void;
};

function CommandMenu({
  items,
  label = "命令面板",
  className,
}: {
  items: CommandItem[];
  label?: string;
  className?: string;
}) {
  return (
    <Command
      className={cn(
        "overflow-hidden rounded-md border border-border bg-popover",
        className,
      )}
      label={label}
    >
      <div className="flex items-center border-b border-border px-3">
        <Search className="size-4 text-muted-foreground" />
        <Command.Input
          className="h-10 w-full bg-transparent px-2 text-sm outline-none placeholder:text-muted-foreground"
          placeholder="搜索命令"
        />
      </div>
      <Command.List className="max-h-56 overflow-auto p-1">
        <Command.Empty className="px-3 py-4 text-sm text-muted-foreground">
          没有可用命令
        </Command.Empty>
        {items.map((item) => (
          <Command.Item
            className="cursor-pointer rounded px-3 py-2 text-sm aria-selected:bg-accent"
            key={item.id}
            keywords={item.keywords ? [item.keywords] : undefined}
            onSelect={item.onSelect}
            value={item.label}
          >
            {item.label}
          </Command.Item>
        ))}
      </Command.List>
    </Command>
  );
}

export { CommandMenu };
export type { CommandItem };
