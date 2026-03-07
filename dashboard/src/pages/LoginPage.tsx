import { useCallback, useEffect, useRef, useState } from "react";
import { useAuth } from "@/lib/auth.tsx";
import { requestOtp, verifyOtp } from "@/api/endpoints.ts";

type Stage = "email" | "otp";

function maskEmail(email: string): string {
  const [local, domain] = email.split("@");
  if (!local || !domain) return email;
  const visible = local.slice(0, 1);
  return `${visible}***@${domain}`;
}

export default function LoginPage() {
  const { login } = useAuth();
  const [stage, setStage] = useState<Stage>("email");
  const [email, setEmail] = useState("");
  const [otp, setOtp] = useState<string[]>(["", "", "", "", "", ""]);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [resendCooldown, setResendCooldown] = useState(0);
  const inputRefs = useRef<(HTMLInputElement | null)[]>([]);

  useEffect(() => {
    if (resendCooldown <= 0) return;
    const timer = setTimeout(() => setResendCooldown((c) => c - 1), 1000);
    return () => clearTimeout(timer);
  }, [resendCooldown]);

  const handleRequestOtp = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      setError("");
      setLoading(true);
      try {
        await requestOtp({ email });
        setStage("otp");
        setResendCooldown(60);
      } catch {
        setError("Failed to send login code. Please try again.");
      } finally {
        setLoading(false);
      }
    },
    [email],
  );

  const handleVerify = useCallback(async () => {
    const code = otp.join("");
    if (code.length !== 6) return;
    setError("");
    setLoading(true);
    try {
      const res = await verifyOtp({ email, otp: code });
      await login(res.token);
    } catch {
      setError("Invalid or expired code. Please try again.");
      setOtp(["", "", "", "", "", ""]);
      inputRefs.current[0]?.focus();
    } finally {
      setLoading(false);
    }
  }, [otp, email, login]);

  const handleOtpChange = useCallback(
    (index: number, value: string) => {
      if (!/^\d*$/.test(value)) return;
      const digit = value.slice(-1);
      const next = [...otp];
      next[index] = digit;
      setOtp(next);

      if (digit && index < 5) {
        inputRefs.current[index + 1]?.focus();
      }

      if (digit && index === 5) {
        const code = next.join("");
        if (code.length === 6) {
          void handleVerify();
        }
      }
    },
    [otp, handleVerify],
  );

  const handleOtpKeyDown = useCallback(
    (index: number, e: React.KeyboardEvent<HTMLInputElement>) => {
      if (e.key === "Backspace" && !otp[index] && index > 0) {
        inputRefs.current[index - 1]?.focus();
      }
    },
    [otp],
  );

  const handleOtpPaste = useCallback(
    (e: React.ClipboardEvent) => {
      e.preventDefault();
      const pasted = e.clipboardData.getData("text").replace(/\D/g, "").slice(0, 6);
      if (!pasted) return;
      const next = [...otp];
      for (let i = 0; i < pasted.length; i++) {
        next[i] = pasted[i];
      }
      setOtp(next);
      const focusIdx = Math.min(pasted.length, 5);
      inputRefs.current[focusIdx]?.focus();
      if (pasted.length === 6) {
        void handleVerify();
      }
    },
    [otp, handleVerify],
  );

  const handleResend = useCallback(async () => {
    if (resendCooldown > 0) return;
    setError("");
    try {
      await requestOtp({ email });
      setResendCooldown(60);
      setOtp(["", "", "", "", "", ""]);
    } catch {
      setError("Failed to resend code.");
    }
  }, [email, resendCooldown]);

  return (
    <div className="min-h-screen bg-gray-50 flex items-center justify-center p-4">
      <div className="w-full max-w-sm bg-white rounded-lg shadow-sm border border-gray-200 p-8">
        {stage === "email" ? (
          <form onSubmit={(e) => void handleRequestOtp(e)}>
            <h2 className="text-2xl font-semibold text-gray-900">School Run</h2>
            <p className="text-gray-500 text-sm mt-1 mb-6">Trading Backtester</p>

            <label className="block text-sm font-medium text-gray-700 mb-1">
              Email
            </label>
            <input
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              required
              autoFocus
              placeholder="you@example.com"
              className="w-full rounded-md border border-gray-200 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
            />

            {error && (
              <p className="mt-2 text-sm text-red-600">{error}</p>
            )}

            <button
              type="submit"
              disabled={loading || !email}
              className="mt-4 w-full rounded-md bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            >
              {loading ? "Sending..." : "Send login code"}
            </button>
          </form>
        ) : (
          <div>
            <h2 className="text-2xl font-semibold text-gray-900">
              Enter your code
            </h2>
            <p className="text-gray-500 text-sm mt-1 mb-6">
              We sent a 6-digit code to {maskEmail(email)}
            </p>

            <div className="flex gap-2 justify-center" onPaste={handleOtpPaste}>
              {otp.map((digit, i) => (
                <input
                  key={i}
                  ref={(el) => {
                    inputRefs.current[i] = el;
                  }}
                  type="text"
                  inputMode="numeric"
                  maxLength={1}
                  value={digit}
                  onChange={(e) => handleOtpChange(i, e.target.value)}
                  onKeyDown={(e) => handleOtpKeyDown(i, e)}
                  autoFocus={i === 0}
                  className="w-10 h-12 text-center text-lg font-mono border border-gray-200 rounded-md focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
                />
              ))}
            </div>

            {error && (
              <p className="mt-3 text-sm text-red-600 text-center">{error}</p>
            )}

            <button
              onClick={() => void handleVerify()}
              disabled={loading || otp.join("").length !== 6}
              className="mt-4 w-full rounded-md bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            >
              {loading ? "Verifying..." : "Verify"}
            </button>

            <p className="mt-4 text-center text-sm text-gray-500">
              {"Didn't get it? "}
              {resendCooldown > 0 ? (
                <span>Resend in {resendCooldown}s</span>
              ) : (
                <button
                  onClick={() => void handleResend()}
                  className="text-blue-600 hover:text-blue-700 font-medium"
                >
                  Resend
                </button>
              )}
            </p>

            <button
              onClick={() => {
                setStage("email");
                setOtp(["", "", "", "", "", ""]);
                setError("");
              }}
              className="mt-2 w-full text-center text-sm text-gray-500 hover:text-gray-700"
            >
              Use a different email
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
