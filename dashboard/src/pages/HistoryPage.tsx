import { useState, useMemo } from "react";
import { useNavigate } from "react-router-dom";
import {
  useReactTable,
  getCoreRowModel,
  getSortedRowModel,
  flexRender,
  createColumnHelper,
} from "@tanstack/react-table";
import type { SortingState } from "@tanstack/react-table";
import { useBacktestHistory } from "@/hooks/useBacktest.ts";
import type { BacktestRunSummary } from "@/types/index.ts";

const columnHelper = createColumnHelper<BacktestRunSummary>();

const columns = [
  columnHelper.accessor("id", {
    header: "Run ID",
    cell: (info) => (
      <span className="font-mono text-xs text-gray-600">{info.getValue().slice(0, 8)}...</span>
    ),
    size: 110,
  }),
  columnHelper.accessor("instrument_id", {
    header: "Instrument",
    cell: (info) => {
      const idMap: Record<number, string> = {
        1: "DAX",
        2: "FTSE",
        3: "IXIC",
        4: "DJI",
      };
      return idMap[info.getValue()] ?? `#${info.getValue()}`;
    },
    size: 100,
  }),
  columnHelper.accessor((row) => `${row.start_date} - ${row.end_date}`, {
    id: "date_range",
    header: "Date Range",
    size: 180,
  }),
  columnHelper.accessor("total_trades", {
    header: "Trades",
    cell: (info) => <span className="font-mono tabular-nums">{info.getValue()}</span>,
    size: 80,
  }),
  columnHelper.accessor("duration_ms", {
    header: "Duration",
    cell: (info) => (
      <span className="font-mono tabular-nums text-gray-500">{info.getValue()}ms</span>
    ),
    size: 90,
  }),
  columnHelper.accessor("created_at", {
    header: "Created",
    cell: (info) => (
      <span className="text-gray-500 text-xs">{new Date(info.getValue()).toLocaleString()}</span>
    ),
    size: 160,
  }),
];

export default function HistoryPage() {
  const navigate = useNavigate();
  const [page, setPage] = useState(0);
  const [sorting, setSorting] = useState<SortingState>([]);
  const { data, isLoading } = useBacktestHistory(page, 25);

  const rows = useMemo(() => data?.data ?? [], [data]);
  const pagination = data?.pagination;

  const table = useReactTable({
    data: rows,
    columns,
    state: { sorting },
    onSortingChange: setSorting,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
  });

  return (
    <div>
      <h2 className="text-lg font-semibold text-gray-900 mb-4">Backtest History</h2>

      {isLoading ? (
        <div className="flex items-center justify-center h-40 text-gray-400 text-sm">
          Loading history...
        </div>
      ) : rows.length === 0 ? (
        <div className="flex items-center justify-center h-40 text-gray-400 text-sm">
          No backtest runs yet
        </div>
      ) : (
        <>
          <div className="bg-gray-50 rounded-lg border border-gray-200 overflow-hidden">
            <table className="w-full text-sm">
              <thead>
                {table.getHeaderGroups().map((hg) => (
                  <tr key={hg.id} className="border-b border-gray-200 bg-white">
                    {hg.headers.map((header) => (
                      <th
                        key={header.id}
                        onClick={header.column.getToggleSortingHandler()}
                        className="text-left py-2.5 px-3 text-xs font-medium text-gray-500 cursor-pointer select-none hover:text-gray-700"
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
                  <tr
                    key={row.id}
                    onClick={() => void navigate(`/backtest/${row.original.id}`)}
                    className="border-b border-gray-100 hover:bg-blue-50/50 cursor-pointer"
                  >
                    {row.getVisibleCells().map((cell) => (
                      <td key={cell.id} className="py-2 px-3 text-gray-700">
                        {flexRender(cell.column.columnDef.cell, cell.getContext())}
                      </td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          {pagination && pagination.total_pages > 1 && (
            <div className="flex items-center justify-between mt-4 text-sm text-gray-500">
              <span>
                Page {page + 1} of {pagination.total_pages}
              </span>
              <div className="flex gap-2">
                <button
                  onClick={() => setPage((p) => Math.max(0, p - 1))}
                  disabled={page === 0}
                  className="px-3 py-1 rounded border border-gray-200 hover:bg-gray-100 disabled:opacity-40 disabled:cursor-not-allowed"
                >
                  Prev
                </button>
                <button
                  onClick={() => setPage((p) => Math.min(pagination.total_pages - 1, p + 1))}
                  disabled={page >= pagination.total_pages - 1}
                  className="px-3 py-1 rounded border border-gray-200 hover:bg-gray-100 disabled:opacity-40 disabled:cursor-not-allowed"
                >
                  Next
                </button>
              </div>
            </div>
          )}
        </>
      )}
    </div>
  );
}
