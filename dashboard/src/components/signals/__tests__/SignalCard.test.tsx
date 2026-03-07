import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import SignalCard from "@/components/signals/SignalCard.tsx";
import type { SignalRow } from "@/types/index.ts";

function makeSignal(overrides: Partial<SignalRow> = {}): SignalRow {
  return {
    id: "abc-123",
    instrument_id: 1,
    signal_date: "2024-01-15",
    signal_bar_high: "18050.00",
    signal_bar_low: "17950.00",
    buy_level: "18052.00",
    sell_level: "17948.00",
    status: "pending",
    fill_details: null,
    created_at: "2024-01-15T08:30:00Z",
    ...overrides,
  };
}

describe("SignalCard", () => {
  it("should render instrument name", () => {
    render(<SignalCard instrumentName="DAX 40" signal={makeSignal()} />);
    expect(screen.getByText("DAX 40")).toBeInTheDocument();
  });

  it("should show signal date when signal exists", () => {
    render(<SignalCard instrumentName="DAX" signal={makeSignal()} />);
    expect(screen.getByText("2024-01-15")).toBeInTheDocument();
  });

  it("should show signal bar high and low", () => {
    render(<SignalCard instrumentName="DAX" signal={makeSignal()} />);
    expect(screen.getByText("18050.00")).toBeInTheDocument();
    expect(screen.getByText("17950.00")).toBeInTheDocument();
  });

  it("should show buy and sell levels", () => {
    render(<SignalCard instrumentName="DAX" signal={makeSignal()} />);
    expect(screen.getByText("18052.00")).toBeInTheDocument();
    expect(screen.getByText("17948.00")).toBeInTheDocument();
  });

  it("should show 'Awaiting signal bar' when no signal", () => {
    render(<SignalCard instrumentName="FTSE" signal={undefined} />);
    expect(screen.getByText("Awaiting signal bar")).toBeInTheDocument();
  });

  it("should not show status badge when no signal", () => {
    render(<SignalCard instrumentName="FTSE" signal={undefined} />);
    expect(screen.queryByText("pending")).not.toBeInTheDocument();
    expect(screen.queryByText("filled")).not.toBeInTheDocument();
    expect(screen.queryByText("expired")).not.toBeInTheDocument();
  });

  it("should show pending status badge with amber styling", () => {
    render(
      <SignalCard instrumentName="DAX" signal={makeSignal({ status: "pending" })} />,
    );
    const badge = screen.getByText("pending");
    expect(badge).toBeInTheDocument();
    expect(badge.className).toContain("bg-amber-100");
    expect(badge.className).toContain("text-amber-800");
  });

  it("should show filled status badge with emerald styling", () => {
    render(<SignalCard instrumentName="DAX" signal={makeSignal({ status: "filled" })} />);
    const badge = screen.getByText("filled");
    expect(badge).toBeInTheDocument();
    expect(badge.className).toContain("bg-emerald-100");
    expect(badge.className).toContain("text-emerald-800");
  });

  it("should show expired status badge with gray styling", () => {
    render(<SignalCard instrumentName="DAX" signal={makeSignal({ status: "expired" })} />);
    const badge = screen.getByText("expired");
    expect(badge).toBeInTheDocument();
    expect(badge.className).toContain("bg-gray-100");
    expect(badge.className).toContain("text-gray-600");
  });

  it("should render High and Low labels", () => {
    render(<SignalCard instrumentName="DAX" signal={makeSignal()} />);
    expect(screen.getByText("High")).toBeInTheDocument();
    expect(screen.getByText("Low")).toBeInTheDocument();
  });

  it("should render Buy Level and Sell Level labels", () => {
    render(<SignalCard instrumentName="DAX" signal={makeSignal()} />);
    expect(screen.getByText("Buy Level")).toBeInTheDocument();
    expect(screen.getByText("Sell Level")).toBeInTheDocument();
  });
});
