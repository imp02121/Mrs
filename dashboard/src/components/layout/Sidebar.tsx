import { NavLink } from "react-router-dom";

const NAV_ITEMS = [
  { to: "/", label: "Backtest", icon: "\u25B6" },
  { to: "/compare", label: "Compare", icon: "\u2194" },
  { to: "/history", label: "History", icon: "\u231A" },
  { to: "/signals", label: "Signals", icon: "\u26A1" },
  { to: "/data", label: "Data", icon: "\u2630" },
] as const;

export default function Sidebar() {
  return (
    <aside className="fixed left-0 top-0 h-screen w-[220px] border-r border-gray-200 bg-white flex flex-col z-10">
      <div className="px-6 py-5">
        <h1 className="text-xl font-semibold text-gray-900">School Run</h1>
        <p className="text-xs text-gray-500 mt-0.5">Trading Backtester</p>
      </div>
      <nav className="flex-1 mt-2">
        {NAV_ITEMS.map((item) => (
          <NavLink
            key={item.to}
            to={item.to}
            end={item.to === "/"}
            className={({ isActive }) =>
              `flex items-center gap-3 px-6 py-2.5 text-sm transition-colors ${
                isActive
                  ? "border-l-4 border-blue-600 bg-blue-50 text-blue-700 font-medium pl-5"
                  : "text-gray-600 hover:bg-gray-50 hover:text-gray-900 border-l-4 border-transparent pl-5"
              }`
            }
          >
            <span className="text-base">{item.icon}</span>
            {item.label}
          </NavLink>
        ))}
      </nav>
    </aside>
  );
}
