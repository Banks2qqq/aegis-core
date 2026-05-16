'use client';

import React, { useEffect, useMemo, useState } from 'react';
import Link from 'next/link';
import { usePathname, useRouter } from 'next/navigation';
import { 
  Shield, 
  Target, 
  Brain, 
  Terminal, 
  Settings, 
  HelpCircle,
  LogOut,
  Lock,
  Database,
  HeartPulse
} from 'lucide-react';
import ReactMissionModal from '../../components/ReactMissionModal';
import ScoutResultToast from '../../components/ScoutResultToast';
import { saveLastScoutRun, type StoredScoutRun } from '../../lib/scoutStorage';
import { ApiClient, decodeJwtClaims, deriveRoleFromClaims, getAccessToken, logout, setTokens } from '../../lib/api';
import { dispatchReactMissionStarted } from '../../lib/aegisEvents';
import { useToast } from '../../components/Toast';
import ErrorBoundary from '../../components/ErrorBoundary';
import LoadingSpinner from '../../components/LoadingSpinner';
import { AegisWsProvider, useAegisWs } from '../../lib/AegisWsProvider';

function LoginOverlay({ onLoginSuccess }: { onLoginSuccess: () => void }) {
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const api = useMemo(() => new ApiClient(), []);

  const handleLogin = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    setError('');

    try {
      let apiKeyToUse = '';
      if (username.trim() === 'root' && password.trim() === '1234') {
        apiKeyToUse = 'test-key-enterprise';
      } else {
        throw new Error('Invalid credentials');
      }

      const data = await api.request('/api/login', {
        method: 'POST',
        json: { api_key: apiKeyToUse },
      });
      
      setTokens(String(data.access_token || ''), String(data.refresh_token || ''));
      const claims = decodeJwtClaims(String(data.access_token || ''));
      try { localStorage.setItem('aegis_role', deriveRoleFromClaims(claims)); } catch {}

      onLoginSuccess();
    } catch (err: any) {
      setError(err.message || 'Login failed.');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="absolute inset-0 z-[1000] bg-black/60 backdrop-blur-md flex items-center justify-center p-6">
      <div className="relative w-full max-w-md">
        <div className="text-center mb-10">
          <div className="inline-flex items-center justify-center w-16 h-16 rounded-2xl bg-gradient-to-br from-[#ddb7ff] to-[#a4c9ff] mb-6 shadow-[0_0_40px_rgba(221,183,255,0.3)]">
            <Lock className="w-8 h-8 text-black" />
          </div>
          <h1 className="text-5xl font-bold tracking-tighter">System Locked</h1>
          <p className="text-white/40 mt-3 tracking-widest text-sm font-mono">AUTHENTICATION REQUIRED</p>
        </div>

        <div className="glass-card-elite rounded-3xl p-10 border border-white/10 shadow-2xl bg-[#050505]/90">
          <form onSubmit={handleLogin} className="space-y-6">
            <div>
              <label className="block text-xs font-mono tracking-[2px] text-white/50 mb-2">USERNAME</label>
              <input
                type="text"
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                className="w-full bg-black/40 border border-white/10 rounded-2xl px-6 py-4 text-lg placeholder:text-white/30 focus:outline-none focus:border-[#ddb7ff]/60 transition-all"
                placeholder="root"
                required
              />
            </div>
            <div>
              <label className="block text-xs font-mono tracking-[2px] text-white/50 mb-2">PASSWORD</label>
              <input
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                className="w-full bg-black/40 border border-white/10 rounded-2xl px-6 py-4 text-lg placeholder:text-white/30 focus:outline-none focus:border-[#ddb7ff]/60 transition-all"
                placeholder="••••"
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
              className="w-full py-4 bg-[#ddb7ff] text-black rounded-2xl font-bold tracking-[3px] text-sm disabled:opacity-60 hover:bg-white transition-all active:scale-[0.985] mt-4"
            >
              {loading ? 'AUTHENTICATING...' : 'UNLOCK SYSTEM'}
            </button>
          </form>
        </div>
      </div>
    </div>
  );
}

function DashboardShell({ children }: { children: React.ReactNode }) {
  const ws = useAegisWs();
  const pathname = usePathname();
  const router = useRouter();
  const [isMissionModalOpen, setIsMissionModalOpen] = useState(false);
  const [role, setRole] = useState<'operator' | 'admin'>('operator');
  const [permissions, setPermissions] = useState<string[]>([]);
  const [theme, setTheme] = useState<'war' | 'stealth'>('war');
  const [authChecking, setAuthChecking] = useState(true);
  const [overlay, setOverlay] = useState<{ status: number; message: string } | null>(null);
  const [airGapped, setAirGapped] = useState<boolean>(false);
  const [scoutToast, setScoutToast] = useState<StoredScoutRun | null>(null);
  const [scoutLoading, setScoutLoading] = useState(false);
  const { showToast } = useToast();
  const api = useMemo(() => new ApiClient({ timeoutMs: 30000 }), []);
  const scoutApi = useMemo(() => new ApiClient({ timeoutMs: 180000 }), []);

  const allNavItems = [
    { href: '/dashboard/overview', label: 'Overview / War Room', icon: Shield, roles: ['operator', 'admin'] },
    { href: '/dashboard/bdu', label: 'BDU Threat Intel', icon: Database, roles: ['operator', 'admin'] },
    { href: '/dashboard/federation', label: 'Federation', icon: Target, roles: ['operator', 'admin'] },
    { href: '/dashboard/threats', label: 'Threat Intelligence', icon: Target, roles: ['operator', 'admin'] },
    { href: '/dashboard/agents', label: 'ReAct++ Agent', icon: Brain, roles: ['operator', 'admin'] },
    { href: '/dashboard/healing', label: 'Healing / HITL', icon: HeartPulse, roles: ['operator', 'admin'] },
    { href: '/dashboard/godmode', label: 'God Mode', icon: Terminal, roles: ['admin'] },
  ];

  const navItems = allNavItems.filter(item => item.roles.includes(role));

  useEffect(() => {
    let mounted = true;
    // Derive role/permissions from JWT claims (validated server-side via /api/status below)
    const token = getAccessToken() || '';
    const claims = token ? decodeJwtClaims(token) : null;
    
    // Defer state updates to avoid synchronous setState warning
    setTimeout(() => {
      if (mounted) {
        setRole(deriveRoleFromClaims(claims));
        setPermissions(Array.isArray(claims?.scope) ? (claims!.scope as string[]) : []);
      }
    }, 0);
    
    return () => { mounted = false; };
  }, []);

  useEffect(() => {
    const handler = (e: any) => {
      const detail = e?.detail;
      if (!detail) return;
      const status = Number(detail.status);
      const message = String(detail.message || '');
      if (!status) return;
      setOverlay({ status, message });
    };
    const unauthHandler = () => {
      setShowLoginOverlay(true);
    };
    window.addEventListener('aegis_api_error', handler as any);
    window.addEventListener('aegis_unauthorized', unauthHandler);
    return () => {
      window.removeEventListener('aegis_api_error', handler as any);
      window.removeEventListener('aegis_unauthorized', unauthHandler);
    };
  }, []);

  const [showLoginOverlay, setShowLoginOverlay] = useState(false);

  // Auth guard + token validation via protected API call
  useEffect(() => {
    let mounted = true;
    const token = getAccessToken();

    if (!token) {
      if (mounted) {
        setShowLoginOverlay(true);
        setAuthChecking(false);
      }
      return;
    }

    (async () => {
      try {
        if (mounted) setAuthChecking(true);
        const s: any = await api.getStatus(); // JWT required; 401 triggers logout
        if (mounted) setAirGapped(Boolean(s?.air_gapped));
      } catch (e: any) {
        // If 401, ApiClient will clear token. We should show overlay.
        if (mounted && !getAccessToken()) {
           setShowLoginOverlay(true);
        }
        console.warn('[AEGIS] status check failed:', e?.message || e);
      } finally {
        if (mounted) setAuthChecking(false);
      }
    })();
    return () => { mounted = false; };
  }, [api, pathname, router]);

  useEffect(() => {
    const openReact = () => setIsMissionModalOpen(true);
    window.addEventListener('aegis_open_react_modal', openReact);
    return () => window.removeEventListener('aegis_open_react_modal', openReact);
  }, []);

  const handleMissionSuccess = (mission: string) => {
    dispatchReactMissionStarted(mission);
    showToast('ReAct++ миссия запущена — смотрите War Room');
    if (pathname !== '/dashboard/overview') {
      router.push('/dashboard/overview');
    }
  };

  return (
    <div className="flex h-screen w-full overflow-hidden bg-[#16111b] text-[#eadfed]">
      {overlay && (
        <div className="fixed inset-0 z-[500] bg-black/80 backdrop-blur-sm flex items-center justify-center p-6">
          <div className="glass-card rounded-3xl p-10 w-full max-w-xl border border-[#ffb4ab]/30">
            <div className="font-mono text-xs tracking-[4px] text-[#ffb4ab] mb-3">
              API ERROR • HTTP {overlay.status}
            </div>
            <div className="text-white/70 font-mono text-sm whitespace-pre-wrap">{overlay.message}</div>
            <div className="mt-6 flex gap-3">
              <button
                type="button"
                onClick={() => setOverlay(null)}
                className="px-6 py-2 text-xs font-mono tracking-widest border border-white/20 rounded-xl hover:bg-white/5 transition-colors"
              >
                DISMISS
              </button>
              <button
                type="button"
                onClick={() => {
                  setOverlay(null);
                  window.location.reload();
                }}
                className="px-6 py-2 text-xs font-mono tracking-widest bg-[#ddb7ff] text-black rounded-xl hover:bg-white transition-colors active:scale-[0.985]"
              >
                RELOAD
              </button>
            </div>
          </div>
        </div>
      )}
      {/* Sidebar */}
      <aside className="flex flex-col h-full w-64 bg-[#110c15]/80 backdrop-blur-xl border-r border-white/10 z-50">
        <div className="px-6 py-8">
          <div className="flex items-center gap-3 mb-2">
            <div className="w-8 h-8 rounded-lg bg-gradient-to-br from-[#ddb7ff] to-[#a4c9ff] flex items-center justify-center">
              <Shield className="w-4.5 h-4.5 text-black" />
            </div>
            <div>
              <div className="font-semibold tracking-tighter text-xl">AEGIS</div>
              <div className="text-[10px] text-white/40 font-mono tracking-[2px]">TERMINAL</div>
            </div>
          </div>
          <div className="text-[10px] text-[#fabc4e] font-mono tracking-widest mt-1">LEVEL 4 CLEARANCE • {role.toUpperCase()}</div>
        </div>

        <nav className="flex-1 px-3 space-y-1">
          {navItems.map((item) => {
            const Icon = item.icon;
            const active = pathname === item.href;
            return (
              <Link
                key={item.href}
                href={item.href}
                className={`flex items-center gap-3 px-4 py-3 rounded-xl text-sm font-medium transition-all ${
                  active 
                    ? 'bg-[#ddb7ff]/10 text-[#ddb7ff] border-r-2 border-[#ddb7ff]' 
                    : 'text-white/60 hover:text-white hover:bg-white/5'
                }`}
              >
                <Icon className="w-4 h-4" />
                <span className="font-mono text-xs tracking-widest">{item.label}</span>
              </Link>
            );
          })}
        </nav>

        <div className="p-4 border-t border-white/10 mt-auto">
          <div className="mt-4 space-y-1 text-xs">
            <Link href="#" className="flex items-center gap-2 px-3 py-2 text-white/50 hover:text-white transition-colors">
              <Settings className="w-3.5 h-3.5" /> Settings
            </Link>
            <Link href="#" className="flex items-center gap-2 px-3 py-2 text-white/50 hover:text-white transition-colors">
              <HelpCircle className="w-3.5 h-3.5" /> Support
            </Link>
            <button 
              onClick={() => {
                logout();
              }}
              className="flex items-center gap-2 px-3 py-2 text-white/50 hover:text-[#ffb4ab] transition-colors w-full text-left"
            >
              <LogOut className="w-3.5 h-3.5" /> Sign Out
            </button>
          </div>
        </div>
      </aside>

      {/* Main Content Area */}
      <div className="flex-1 flex flex-col min-w-0">
        {/* Top Bar */}
        <header className="h-16 border-b border-white/10 bg-[#16111b]/60 backdrop-blur-xl flex items-center justify-between px-8 z-40">
          <div className="flex items-center gap-6">
            <div className="flex items-center gap-2">
              <div className="w-2 h-2 rounded-full bg-[#fabc4e] animate-pulse" />
              <span className="font-mono text-xs tracking-[3px] text-[#fabc4e]">ORACLE: OPERATIONAL</span>
            </div>
            <div className="h-3 w-px bg-white/20" />
            <div className={`flex items-center gap-2 text-xs font-mono tracking-widest ${ws.status === 'connected' ? 'text-[#00F5A3]' : 'text-[#ffb4ab]'}`}>
              <span className={`w-2 h-2 rounded-full ${ws.status === 'connected' ? 'bg-[#00F5A3] animate-pulse' : 'bg-[#ffb4ab]'}`} />
              LIVE {ws.status === 'connected' ? 'ON' : 'OFF'}
            </div>
            {airGapped && (
              <>
                <div className="h-3 w-px bg-white/20" />
                <div className="px-3 py-1 rounded-full bg-white/5 border border-white/10 text-xs font-mono tracking-widest text-[#00F5A3]">
                  AIR-GAPPED: ENABLED
                </div>
              </>
            )}
            <div className="h-3 w-px bg-white/20" />
            <div className="font-mono text-[11px] text-white/50">
              SESSION: <span className="text-[#ddb7ff]">X-9942-ALPHA</span>
            </div>
            {permissions.length > 0 && (
              <>
                <div className="h-3 w-px bg-white/20" />
                <div className="font-mono text-[11px] text-white/50">
                  SCOPE: <span className="text-white/70">{permissions.slice(0, 3).join(', ')}</span>
                </div>
              </>
            )}
          </div>

          <div className="flex items-center gap-3">
            <button
              type="button"
              disabled={scoutLoading}
              onClick={async () => {
                setScoutLoading(true);
                try {
                  const res = await scoutApi.runScout();
                  if (res.status === 'success') {
                    const stored = saveLastScoutRun(res);
                    setScoutToast(stored);
                    window.setTimeout(() => setScoutToast(null), 12000);
                  } else {
                    setOverlay({ status: 502, message: res.error || 'SCOUT: ошибка цикла' });
                  }
                } catch (e: unknown) {
                  const err = e as Error & { httpStatus?: number };
                  const msg = err?.message || 'Scout failed';
                  if (err?.httpStatus === 409 || /уже выполняется/i.test(msg)) {
                    setOverlay({
                      status: 409,
                      message: 'SCOUT уже выполняется — дождитесь завершения предыдущего цикла',
                    });
                  } else if (err?.httpStatus === 504 || /timeout/i.test(msg)) {
                    setOverlay({ status: 504, message: 'SCOUT: таймаут цикла (проверьте LLM и источники)' });
                  } else {
                    setOverlay({ status: err?.httpStatus || 500, message: msg });
                  }
                } finally {
                  setScoutLoading(false);
                }
              }}
              className="px-4 py-1.5 text-xs border border-[#00F5A3]/40 text-[#00F5A3] rounded-lg hover:bg-[#00F5A3]/10 font-mono tracking-widest transition-all active:scale-[0.985] disabled:opacity-50"
            >
              {scoutLoading ? 'SCOUT…' : 'SCOUT'}
            </button>
            <button
              type="button"
              onClick={() => setIsMissionModalOpen(true)}
              className="px-4 py-1.5 text-xs border border-[#ddb7ff]/40 text-[#ddb7ff] rounded-lg hover:bg-[#ddb7ff]/10 font-mono tracking-widest transition-all active:scale-[0.985]"
            >
              REACT MISSION
            </button>
            <button
              type="button"
              onClick={() => {
                const next = theme === 'war' ? 'stealth' : 'war';
                setTheme(next);
                document.documentElement.classList.toggle('stealth-mode', next === 'stealth');
              }}
              className="px-4 py-1.5 text-xs border border-white/20 rounded-lg hover:bg-white/5 font-mono tracking-widest transition-all active:scale-[0.985]"
            >
              {theme === 'war' ? 'STEALTH MODE' : 'WAR ROOM'}
            </button>
          </div>
        </header>

        {/* Page Content */}
        <main className="flex-1 overflow-auto p-8 bg-[radial-gradient(circle_at_50%_50%,rgba(132,43,210,0.04)_0%,transparent_70%)] relative">
          {authChecking ? (
            <div className="flex h-[60vh] items-center justify-center">
              <LoadingSpinner label="Validating session..." />
            </div>
          ) : (
            <ErrorBoundary fallbackTitle="DASHBOARD UI ERROR">
              {children}
            </ErrorBoundary>
          )}
          {showLoginOverlay && (
            <LoginOverlay onLoginSuccess={() => {
              setShowLoginOverlay(false);
              setAuthChecking(true);
              api.getStatus().then((s: { air_gapped?: boolean }) => {
                setAirGapped(Boolean(s?.air_gapped));
              }).catch(() => {}).finally(() => setAuthChecking(false));
            }} />
          )}
        </main>
      </div>

      <ReactMissionModal 
        isOpen={isMissionModalOpen} 
        onClose={() => setIsMissionModalOpen(false)} 
        onSuccess={handleMissionSuccess} 
      />
      <ScoutResultToast result={scoutToast} onClose={() => setScoutToast(null)} />
    </div>
  );
}

export default function DashboardLayout({ children }: { children: React.ReactNode }) {
  return (
    <AegisWsProvider>
      <DashboardShell>{children}</DashboardShell>
    </AegisWsProvider>
  );
}
