import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  // Phase 6: required by deploy/Dockerfile.web — produces minimal Node.js
  // runtime under .next/standalone for the runner stage.
  output: "standalone",
  // Public API calls remain relative and traverse the same-origin gateway.
  // Server-side proxy code resolves INTERNAL_API_URL at container runtime.
};

export default nextConfig;
