import {
  flexRender,
  getCoreRowModel,
  getFilteredRowModel,
  type ColumnDef,
  useReactTable,
} from "@tanstack/react-table";
import { Search } from "lucide-react";
import * as React from "react";
import { Input } from "./input";
import { cn } from "../lib/utils";

type DataTableProps<TData> = {
  columns: ColumnDef<TData, unknown>[];
  data: TData[];
  emptyLabel: string;
  filterPlaceholder?: string;
  getRowId?: (row: TData, index: number) => string;
  onRowClick?: (row: TData) => void;
  className?: string;
};

function DataTable<TData>({
  columns,
  data,
  emptyLabel,
  filterPlaceholder,
  getRowId,
  onRowClick,
  className,
}: DataTableProps<TData>) {
  const [globalFilter, setGlobalFilter] = React.useState("");
  const table = useReactTable({
    columns,
    data,
    getCoreRowModel: getCoreRowModel(),
    getFilteredRowModel: getFilteredRowModel(),
    getRowId,
    state: { globalFilter },
    onGlobalFilterChange: setGlobalFilter,
  });

  return (
    <div className={cn("grid gap-3", className)}>
      {filterPlaceholder && (
        <label className="relative block max-w-sm">
          <Search className="pointer-events-none absolute top-3 left-3 size-4 text-muted-foreground" />
          <Input
            aria-label={filterPlaceholder}
            className="pl-9"
            placeholder={filterPlaceholder}
            value={globalFilter}
            onChange={(event) => setGlobalFilter(event.target.value)}
          />
        </label>
      )}
      <div className="overflow-x-auto rounded-md border border-border">
        <table className="w-full min-w-[38rem] border-collapse text-left text-sm">
          <thead className="bg-muted text-muted-foreground">
            {table.getHeaderGroups().map((group) => (
              <tr key={group.id}>
                {group.headers.map((header) => (
                  <th className="px-3 py-2 font-medium" key={header.id}>
                    {header.isPlaceholder
                      ? null
                      : flexRender(
                          header.column.columnDef.header,
                          header.getContext(),
                        )}
                  </th>
                ))}
              </tr>
            ))}
          </thead>
          <tbody>
            {table.getRowModel().rows.map((row) => (
              <tr
                className={cn(
                  "border-t border-border",
                  onRowClick && "cursor-pointer hover:bg-accent/60",
                )}
                key={row.id}
                onClick={() => onRowClick?.(row.original)}
              >
                {row.getVisibleCells().map((cell) => (
                  <td className="px-3 py-2 align-top" key={cell.id}>
                    {flexRender(cell.column.columnDef.cell, cell.getContext())}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
        {table.getRowModel().rows.length === 0 && (
          <p className="px-3 py-8 text-center text-sm text-muted-foreground">
            {emptyLabel}
          </p>
        )}
      </div>
    </div>
  );
}

export { DataTable };
export type { DataTableProps };
