/** Request body for `POST /auth/request-otp`. */
export interface RequestOtpBody {
  email: string;
}

/** Request body for `POST /auth/verify-otp`. */
export interface VerifyOtpBody {
  email: string;
  otp: string;
}

/** Successful response from `POST /auth/verify-otp`. */
export interface VerifyOtpResponse {
  token: string;
  /** ISO 8601 UTC timestamp */
  expires_at: string;
}

/** Successful response from `GET /auth/me`. */
export interface MeResponse {
  email: string;
  role: string;
}

/** Generic message response from auth service. */
export interface MessageResponse {
  message: string;
}
