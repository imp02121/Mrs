import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom";
import { AuthProvider, useAuth } from "@/lib/auth.tsx";
import AppShell from "@/components/layout/AppShell.tsx";
import LoginPage from "@/pages/LoginPage.tsx";
import BacktestPage from "@/pages/BacktestPage.tsx";
import BacktestDetailPage from "@/pages/BacktestDetailPage.tsx";
import ComparePage from "@/pages/ComparePage.tsx";
import HistoryPage from "@/pages/HistoryPage.tsx";
import SignalsPage from "@/pages/SignalsPage.tsx";
import DataPage from "@/pages/DataPage.tsx";

function AppRoutes() {
  const { isAuthenticated, isLoading } = useAuth();

  if (isLoading) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-white">
        <p className="text-gray-400 text-sm">Loading...</p>
      </div>
    );
  }

  if (!isAuthenticated) {
    return <LoginPage />;
  }

  return (
    <Routes>
      <Route element={<AppShell />}>
        <Route path="/" element={<BacktestPage />} />
        <Route path="/backtest/:id" element={<BacktestDetailPage />} />
        <Route path="/compare" element={<ComparePage />} />
        <Route path="/history" element={<HistoryPage />} />
        <Route path="/signals" element={<SignalsPage />} />
        <Route path="/data" element={<DataPage />} />
      </Route>
      <Route path="*" element={<Navigate to="/" replace />} />
    </Routes>
  );
}

export default function App() {
  return (
    <BrowserRouter>
      <AuthProvider>
        <AppRoutes />
      </AuthProvider>
    </BrowserRouter>
  );
}
