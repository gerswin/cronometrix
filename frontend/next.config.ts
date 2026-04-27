import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  // Phase 6: required by deploy/Dockerfile.web — produces minimal Node.js
  // runtime under .next/standalone for the runner stage.
  output: "standalone",
};

export default nextConfig;
