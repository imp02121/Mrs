import { useState } from "react";
import {
  useReactTable,
  getCoreRowModel,
  getSortedRowModel,
  getPaginationRowModel,
  flexRender,
  createColumnHelper,
} from "@tanstack/react-table";
import type { SortingState } from "@tanstack/react-table";
import type { Trade } from "@/types/index.ts";

interface TradeTableProps {
  trades: Trade[];
}

const columnHelper = createColumnHelper<Trade>();

const columns = [
  columnHelper.display({
    id: "index",
    header: "#",
    cell: (info) => info.row.index + 1,
    size: 50,
  }),
  columnHelper.accessor((row) => row.entry_time.slice(0, 10), {
    id: "date",
    header: "Date",
    size: 100,
  }),
  columnHelper.accessor("direction", {
    header: "Dir",
    size: 70,
    cell: (info) => {
      const dir = info.getValue();
      return (
        <span
          className={`inline-block rounded px-2 py-0.5 text-xs font-medium ${
            dir === "Long" ? "bg-emerald-100 text-emerald-700" : "bg-red-100 text-red-700"
          }`}
        >
          {dir}
        </span>
      );
    },
  }),
  columnHelper.accessor("entry_price", {
    header: "Entry",
    size: 90,
    cell: (info) => <span className="font-mono tabular-nums">{info.getValue()}</span>,
  }),
  columnHelper.accessor("exit_price", {
    header: "Exit",
    size: 90,
    cell: (info) => <span className="font-mono tabular-nums">{info.getValue()}</span>,
  }),
  columnHelper.accessor("stop_loss", {
    header: "SL",
    size: 90,
    cell: (info) => <span className="font-mono tabular-nums">{info.getValue()}</span>,
  }),
  columnHelper.accessor("pnl_points", {
    header: "PnL",
    size: 90,
    cell: (info) => {
      const val = parseFloat(info.getValue());
      return (
        <span
          className={`font-mono tabular-nums font-medium ${
            val >= 0 ? "text-emerald-600" : "text-red-600"
          }`}
        >
          {val >= 0 ? `+${info.getValue()}` : info.getValue()}
        </span>
      );
    },
    sortingFn: (a, b) => parseFloat(a.original.pnl_points) - parseFloat(b.original.pnl_points),
  }),
  columnHelper.accessor("exit_reason", {
    header: "Exit Reason",
    size: 100,
  }),
];

export default function TradeTable({ trades }: TradeTableProps) {
  const [sorting, setSorting] = useState<SortingState>([]);

  // eslint-disable-next-line react-hooks/incompatible-library
  const table = useReactTable({
    data: trades,
    columns,
    state: { sorting },
    onSortingChange: setSorting,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    getPaginationRowModel: getPaginationRowModel(),
    initialState: {
      pagination: { pageSize: 25 },
    },
  });

  const pageIndex = table.getState().pagination.pageIndex;
  const pageCount = table.getPageCount();

  return (
    <div className="bg-gray-50 rounded-lg border border-gray-200 p-4">
      <h3 className="text-sm font-medium text-gray-700 mb-3">Trades</h3>
      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            {table.getHeaderGroups().map((hg) => (
              <tr key={hg.id} className="border-b border-gray-200">
                {hg.headers.map((header) => (
                  <th
                    key={header.id}
                    onClick={header.column.getToggleSortingHandler()}
                    className="text-left py-2 px-2 text-xs font-medium text-gray-500 cursor-pointer select-none hover:text-gray-700"
                    style={{ width: header.getSize() }}
                  >
                    <div className="flex items-center gap-1">
                      {flexRender(header.column.columnDef.header, header.getContext())}
                      {{
                        asc: " \u2191",
                        desc: " \u2193",
                      }[header.column.getIsSorted() as string] ?? null}
                    </div>
                  </th>
                ))}
              </tr>
            ))}
          </thead>
          <tbody>
            {table.getRowModel().rows.map((row) => (
              <tr key={row.id} className="border-b border-gray-100 hover:bg-gray-100/50">
                {row.getVisibleCells().map((cell) => (
                  <td key={cell.id} className="py-1.5 px-2 text-gray-700">
                    {flexRender(cell.column.columnDef.cell, cell.getContext())}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {pageCount > 1 && (
        <div className="flex items-center justify-between mt-3 text-sm text-gray-500">
          <span>
            Page {pageIndex + 1} of {pageCount}
          </span>
          <div className="flex gap-2">
            <button
              onClick={() => table.previousPage()}
              disabled={!table.getCanPreviousPage()}
              className="px-3 py-1 rounded border border-gray-200 hover:bg-gray-100 disabled:opacity-40 disabled:cursor-not-allowed"
            >
              Prev
            </button>
            <button
              onClick={() => table.nextPage()}
              disabled={!table.getCanNextPage()}
              className="px-3 py-1 rounded border border-gray-200 hover:bg-gray-100 disabled:opacity-40 disabled:cursor-not-allowed"
            >
              Next
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
