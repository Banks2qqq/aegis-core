import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const isTauri = process.env.TAURI_BUILD === 'true' || process.env.NEXT_PUBLIC_IS_TAURI === 'true' || process.env.EXPORT === 'true';

/** @type {import('next').NextConfig} */
const nextConfig = {
  output: 'export',
  
  // Avoid Next.js auto-inferring an incorrect workspace root when multiple lockfiles exist.
  turbopack: {
    root: __dirname,
  },

  // Production optimizations for AEGIS 2026
  poweredByHeader: false,
  // StrictMode double-mount breaks WebSocket on dashboard (console noise + flaky live feed).
  reactStrictMode: false,
  compress: true,

  // Image optimization (add domains if you use external images)
  images: {
    unoptimized: true,
    formats: ['image/avif', 'image/webp'],
    minimumCacheTTL: 31536000,
  },

  // Security headers (also defined in vercel.json for edge)
  // headers: async () => {
  //   return [
  //     {
  //       source: '/:path*',
  //       headers: [
  //         {
  //           key: 'Strict-Transport-Security',
  //           value: 'max-age=63072000; includeSubDomains; preload',
  //         },
  //       ],
  //     },
  //   ];
  // },
};

export default nextConfig;
