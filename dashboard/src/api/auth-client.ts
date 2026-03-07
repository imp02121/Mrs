import axios from "axios";

/** Axios instance for the auth service. */
const authApi = axios.create({
  baseURL: import.meta.env.VITE_AUTH_URL ?? "http://localhost:3002",
  headers: { "Content-Type": "application/json" },
});

// Request interceptor: attach JWT from localStorage
authApi.interceptors.request.use((config) => {
  const token = localStorage.getItem("sr_token");
  if (token) {
    config.headers.Authorization = `Bearer ${token}`;
  }
  return config;
});

export default authApi;
