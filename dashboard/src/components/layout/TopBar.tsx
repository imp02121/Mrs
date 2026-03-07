import { useAuth } from "@/lib/auth.tsx";
import { useUiStore } from "@/stores/ui-store.ts";

export default function TopBar() {
  const { user, logout } = useAuth();
  const toggleSidebar = useUiStore((s) => s.toggleSidebar);

  return (
    <header className="h-14 border-b border-gray-200 bg-white flex items-center justify-between px-6 print:hidden">
      <button
        onClick={toggleSidebar}
        className="md:hidden p-1.5 -ml-1.5 rounded-md text-gray-500 hover:text-gray-900 hover:bg-gray-100 transition-colors"
        aria-label="Toggle menu"
      >
        <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M4 6h16M4 12h16M4 18h16"
          />
        </svg>
      </button>
      <div className="hidden md:block" />
      {user && (
        <div className="flex items-center gap-4">
          <span className="text-sm text-gray-600">{user.email}</span>
          <button
            onClick={() => void logout()}
            className="text-sm text-gray-500 hover:text-gray-900 transition-colors"
          >
            Logout
          </button>
        </div>
      )}
    </header>
  );
}
