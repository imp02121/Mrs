import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import LoginPage from "@/pages/LoginPage.tsx";

// Mock the auth context
const mockLogin = vi.fn();
vi.mock("@/lib/auth.tsx", () => ({
  useAuth: () => ({
    login: mockLogin,
    user: null,
    token: null,
    isAuthenticated: false,
    isLoading: false,
    logout: vi.fn(),
  }),
}));

// Mock the API endpoints
vi.mock("@/api/endpoints.ts", () => ({
  requestOtp: vi.fn(),
  verifyOtp: vi.fn(),
}));

describe("LoginPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("should render the email form initially", () => {
    render(<LoginPage />);

    expect(screen.getByText("School Run")).toBeInTheDocument();
    expect(screen.getByText("Trading Backtester")).toBeInTheDocument();
    expect(screen.getByText("Email")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("you@example.com")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Send login code" })).toBeInTheDocument();
  });

  it("should have a disabled submit button when email is empty", () => {
    render(<LoginPage />);
    const button = screen.getByRole("button", { name: "Send login code" });
    expect(button).toBeDisabled();
  });

  it("should enable submit button when email is entered", async () => {
    const user = userEvent.setup();
    render(<LoginPage />);

    const emailInput = screen.getByPlaceholderText("you@example.com");
    await user.type(emailInput, "test@example.com");

    const button = screen.getByRole("button", { name: "Send login code" });
    expect(button).not.toBeDisabled();
  });

  it("should show error when OTP request fails", async () => {
    const { requestOtp } = await import("@/api/endpoints.ts");
    vi.mocked(requestOtp).mockRejectedValueOnce(new Error("Network error"));

    const user = userEvent.setup();
    render(<LoginPage />);

    const emailInput = screen.getByPlaceholderText("you@example.com");
    await user.type(emailInput, "test@example.com");

    const button = screen.getByRole("button", { name: "Send login code" });
    await user.click(button);

    expect(
      await screen.findByText("Failed to send login code. Please try again."),
    ).toBeInTheDocument();
  });

  it("should transition to OTP stage on successful email submission", async () => {
    const { requestOtp } = await import("@/api/endpoints.ts");
    vi.mocked(requestOtp).mockResolvedValueOnce({ message: "OTP sent" });

    const user = userEvent.setup();
    render(<LoginPage />);

    const emailInput = screen.getByPlaceholderText("you@example.com");
    await user.type(emailInput, "test@example.com");
    await user.click(screen.getByRole("button", { name: "Send login code" }));

    expect(await screen.findByText("Enter your code")).toBeInTheDocument();
    // Email should be masked
    expect(screen.getByText(/t\*\*\*@example\.com/)).toBeInTheDocument();
  });

  it("should render 6 OTP input fields in the OTP stage", async () => {
    const { requestOtp } = await import("@/api/endpoints.ts");
    vi.mocked(requestOtp).mockResolvedValueOnce({ message: "OTP sent" });

    const user = userEvent.setup();
    render(<LoginPage />);

    await user.type(screen.getByPlaceholderText("you@example.com"), "test@example.com");
    await user.click(screen.getByRole("button", { name: "Send login code" }));

    await screen.findByText("Enter your code");
    const otpInputs = screen.getAllByRole("textbox");
    expect(otpInputs).toHaveLength(6);
  });

  it("should have a 'Use a different email' button in OTP stage", async () => {
    const { requestOtp } = await import("@/api/endpoints.ts");
    vi.mocked(requestOtp).mockResolvedValueOnce({ message: "OTP sent" });

    const user = userEvent.setup();
    render(<LoginPage />);

    await user.type(screen.getByPlaceholderText("you@example.com"), "test@example.com");
    await user.click(screen.getByRole("button", { name: "Send login code" }));

    await screen.findByText("Enter your code");
    expect(screen.getByText("Use a different email")).toBeInTheDocument();
  });
});
