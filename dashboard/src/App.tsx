import { BrowserRouter, Routes, Route } from "react-router-dom";
import BacktestPage from "@/pages/BacktestPage";
import ChartPage from "@/pages/ChartPage";
import ComparePage from "@/pages/ComparePage";
import SignalsPage from "@/pages/SignalsPage";
import HistoryPage from "@/pages/HistoryPage";

export default function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<BacktestPage />} />
        <Route path="/chart" element={<ChartPage />} />
        <Route path="/compare" element={<ComparePage />} />
        <Route path="/signals" element={<SignalsPage />} />
        <Route path="/history" element={<HistoryPage />} />
      </Routes>
    </BrowserRouter>
  );
}
