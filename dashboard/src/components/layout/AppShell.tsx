import { Outlet } from "react-router-dom";
import Sidebar from "./Sidebar.tsx";
import TopBar from "./TopBar.tsx";

export default function AppShell() {
  return (
    <div className="flex h-screen bg-white">
      <Sidebar />
      <div className="flex-1 flex flex-col ml-[220px]">
        <TopBar />
        <main className="flex-1 overflow-auto">
          <div className="max-w-[1400px] mx-auto p-6">
            <Outlet />
          </div>
        </main>
      </div>
    </div>
  );
}
