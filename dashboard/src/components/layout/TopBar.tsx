import { useAuth } from "@/lib/auth.tsx";

export default function TopBar() {
  const { user, logout } = useAuth();

  return (
    <header className="h-14 border-b border-gray-200 bg-white flex items-center justify-end px-6">
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
