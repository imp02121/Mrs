import axios from "axios";

/** Axios instance for the engine API. */
const api = axios.create({
  baseURL: import.meta.env.VITE_API_URL ?? "http://localhost:3001/api",
  headers: { "Content-Type": "application/json" },
});

// Request interceptor: attach JWT from localStorage
api.interceptors.request.use((config) => {
  const token = localStorage.getItem("sr_token");
  if (token) {
    config.headers.Authorization = `Bearer ${token}`;
  }
  return config;
});

// Response interceptor: 401 -> clear token and redirect to /login
api.interceptors.response.use(
  (res) => res,
  (err: unknown) => {
    if (axios.isAxiosError(err) && err.response?.status === 401) {
      localStorage.removeItem("sr_token");
      window.location.href = "/login";
    }
    return Promise.reject(err);
  },
);

export default api;
