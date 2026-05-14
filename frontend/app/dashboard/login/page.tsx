'use client';

import React, { useState } from 'react';
import { useRouter } from 'next/navigation';
import { Shield, Lock } from 'lucide-react';
import { ApiClient, decodeJwtClaims, deriveRoleFromClaims, setTokens } from '../../../lib/api';

const api = new ApiClient();

export default function LoginPage() {
  const [apiKey, setApiKey] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const router = useRouter();

  const handleLogin = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    setError('');

    try {
      // Backend expects: POST /api/login { api_key }
      const data = await api.request('/api/login', {
        method: 'POST',
        json: { api_key: apiKey.trim() },
      });
      
      // Store tokens in-memory (Zero-Trust demo mode)
      setTokens(String(data.access_token || ''), String(data.refresh_token || ''));
      // For UI only (RBAC is still enforced server-side by JWT validation).
      const claims = decodeJwtClaims(String(data.access_token || ''));
      // Keep legacy value for display/back-compat; not used for access decisions.
      try { localStorage.setItem('aegis_role', deriveRoleFromClaims(claims)); } catch {}

      // Redirect to dashboard
      router.push('/dashboard/overview');
    } catch (err: any) {
      setError(err.message || 'Login failed. Check API key or backend availability.');
    } finally {
      setLoading(false);
    }
  };

  const quickFillDevKey = (tier: 'starter' | 'pro' | 'enterprise') => {
    // Requires backend env: AEGIS_DEV_MODE=1
    setApiKey(`test-key-${tier}`);
  };

  return (
    <div className="min-h-screen bg-[#050505] flex items-center justify-center px-6 relative overflow-hidden">
      <div className="absolute inset-0 bg-[radial-gradient(#ffffff08_0.5px,transparent_1px)] bg-[length:4px_4px]" />
      
      <div className="relative w-full max-w-md">
        <div className="text-center mb-10">
          <div className="inline-flex items-center justify-center w-16 h-16 rounded-2xl bg-gradient-to-br from-[#ddb7ff] to-[#a4c9ff] mb-6">
            <Shield className="w-8 h-8 text-black" />
          </div>
          <h1 className="text-5xl font-bold tracking-tighter">AEGIS Terminal</h1>
          <p className="text-white/40 mt-3 tracking-widest text-sm font-mono">LEVEL 4 CLEARANCE REQUIRED</p>
        </div>

        <div className="glass-card-elite rounded-3xl p-10 border border-white/10">
          <form onSubmit={handleLogin} className="space-y-6">
            <div>
              <label className="block text-xs font-mono tracking-[2px] text-white/50 mb-2">API KEY</label>
              <input
                type="password"
                value={apiKey}
                onChange={(e) => setApiKey(e.target.value)}
                className="w-full bg-black/40 border border-white/10 rounded-2xl px-6 py-4 text-lg placeholder:text-white/30 focus:outline-none focus:border-[#ddb7ff]/60 transition-all"
                placeholder="test-key-enterprise (dev) / enterprise key (prod)"
                required
              />
            </div>

            {error && (
              <div className="text-[#ffb4ab] text-sm bg-[#ffb4ab]/10 border border-[#ffb4ab]/30 rounded-2xl px-5 py-3">
                {error}
              </div>
            )}

            <button
              type="submit"
              disabled={loading}
              className="w-full py-4 bg-[#ddb7ff] text-black rounded-2xl font-bold tracking-[3px] text-sm disabled:opacity-60 hover:bg-white transition-all active:scale-[0.985]"
            >
              {loading ? 'AUTHENTICATING...' : 'AUTHENTICATE & ENTER WAR ROOM'}
            </button>
          </form>

          <div className="mt-6 text-center space-x-4 text-xs font-mono tracking-widest">
            <button onClick={() => quickFillDevKey('enterprise')} type="button" className="text-[#ddb7ff] hover:text-white underline-offset-4 hover:underline">
              FILL DEV: ENTERPRISE
            </button>
            <span className="text-white/20">|</span>
            <button onClick={() => quickFillDevKey('pro')} type="button" className="text-white/40 hover:text-white underline-offset-4 hover:underline">
              FILL DEV: PRO
            </button>
          </div>
        </div>

        <div className="text-center mt-8 text-[10px] text-white/30 font-mono tracking-[3px]">
          ALL SESSIONS ARE LOGGED • ZERO-TRUST ENFORCED
        </div>
      </div>
    </div>
  );
}
