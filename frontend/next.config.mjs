import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

/** @type {import('next').NextConfig} */
const nextConfig = {
  // Avoid Next.js auto-inferring an incorrect workspace root when multiple lockfiles exist.
  turbopack: {
    root: __dirname,
  },

  // Production optimizations for AEGIS 2026
  poweredByHeader: false,
  reactStrictMode: true,
  compress: true,

  // Image optimization (add domains if you use external images)
  images: {
    formats: ['image/avif', 'image/webp'],
    minimumCacheTTL: 31536000,
  },

  // Security headers (also defined in vercel.json for edge)
  async headers() {
    return [
      {
        source: '/:path*',
        headers: [
          {
            key: 'Strict-Transport-Security',
            value: 'max-age=63072000; includeSubDomains; preload',
          },
        ],
      },
    ];
  },
};

export default nextConfig;
