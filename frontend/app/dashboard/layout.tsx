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
  LogOut
} from 'lucide-react';
import ReactMissionModal from '../../components/ReactMissionModal';
import { ApiClient, decodeJwtClaims, deriveRoleFromClaims, getAccessToken, logout } from '../../lib/api';
import ErrorBoundary from '../../components/ErrorBoundary';
import LoadingSpinner from '../../components/LoadingSpinner';

export default function DashboardLayout({ children }: { children: React.ReactNode }) {
  const pathname = usePathname();
  const router = useRouter();
  const [isMissionModalOpen, setIsMissionModalOpen] = useState(false);
  const [role, setRole] = useState<'operator' | 'admin'>('operator');
  const [permissions, setPermissions] = useState<string[]>([]);
  const [theme, setTheme] = useState<'war' | 'stealth'>('war');
  const [authChecking, setAuthChecking] = useState(true);
  const [overlay, setOverlay] = useState<{ status: number; message: string } | null>(null);
  const [airGapped, setAirGapped] = useState<boolean>(false);
  const api = useMemo(() => new ApiClient({ timeoutMs: 8000 }), []);

  const allNavItems = [
    { href: '/dashboard/overview', label: 'Overview / War Room', icon: Shield, roles: ['operator', 'admin'] },
    { href: '/dashboard/threats', label: 'Threat Intelligence', icon: Target, roles: ['operator', 'admin'] },
    { href: '/dashboard/agents', label: 'ReAct++ Agent', icon: Brain, roles: ['operator', 'admin'] },
    { href: '/dashboard/demo', label: 'Demo Tour', icon: Target, roles: ['operator', 'admin'] },
    { href: '/dashboard/godmode', label: 'God Mode', icon: Terminal, roles: ['admin'] },
  ];

  const navItems = allNavItems.filter(item => item.roles.includes(role));

  useEffect(() => {
    // Derive role/permissions from JWT claims (validated server-side via /api/status below)
    const token = getAccessToken() || '';
    const claims = token ? decodeJwtClaims(token) : null;
    setRole(deriveRoleFromClaims(claims));
    setPermissions(Array.isArray(claims?.scope) ? (claims!.scope as string[]) : []);
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
    window.addEventListener('aegis_api_error', handler as any);
    return () => window.removeEventListener('aegis_api_error', handler as any);
  }, []);

  // Auth guard + token validation via protected API call
  useEffect(() => {
    const token = getAccessToken();
    const isLoginPage = pathname === '/dashboard/login';

    if (!token && !isLoginPage) {
      router.replace('/dashboard/login');
      setAuthChecking(false);
      return;
    }
    if (!token || isLoginPage) return;

    (async () => {
      try {
        setAuthChecking(true);
        const s: any = await api.getStatus(); // JWT required; 401 triggers logout + redirect
        setAirGapped(Boolean(s?.air_gapped));
      } catch (e: any) {
        // ApiClient already handles 401. For other failures, keep user in dashboard but console-log.
        console.warn('[AEGIS] status check failed:', e?.message || e);
      } finally {
        setAuthChecking(false);
      }
    })();
  }, [api, pathname, router]);

  const handleMissionSuccess = (mission: string) => {
    // Could show toast or update some global state here
    console.log('[AEGIS] Mission launched:', mission);
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
                className="px-6 py-2 text-xs font-mono tracking-widest border border-white/20 rounded-xl hover:bg-white/5"
              >
                DISMISS
              </button>
              <button
                type="button"
                onClick={() => {
                  setOverlay(null);
                  window.location.reload();
                }}
                className="px-6 py-2 text-xs font-mono tracking-widest bg-[#ddb7ff] text-black rounded-xl hover:bg-white"
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
          <button 
            onClick={() => setIsMissionModalOpen(true)}
            className="w-full bg-[#ddb7ff] text-black py-3 rounded-xl text-xs font-bold tracking-[2px] hover:bg-white transition-all active:scale-[0.985]"
          >
            DEPLOY REACT++
          </button>
          
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
            <button className="px-5 py-1.5 text-xs border border-white/20 rounded-lg hover:bg-white/5 transition-all font-mono tracking-widest">
              SHARE
            </button>
            <button 
              onClick={() => setIsMissionModalOpen(true)}
              className="px-5 py-1.5 bg-[#ddb7ff] text-black text-xs font-bold tracking-[2px] rounded-lg hover:bg-white transition-all"
            >
              START PILOT
            </button>

            <button
              onClick={() => {
                const next = theme === 'war' ? 'stealth' : 'war';
                setTheme(next);
                document.documentElement.classList.toggle('stealth-mode', next === 'stealth');
              }}
              className="px-4 py-1.5 text-xs border border-white/20 rounded-lg hover:bg-white/5 font-mono tracking-widest"
            >
              {theme === 'war' ? 'STEALTH MODE' : 'WAR ROOM'}
            </button>
          </div>
        </header>

        {/* Page Content */}
        <main className="flex-1 overflow-auto p-8 bg-[radial-gradient(circle_at_50%_50%,rgba(132,43,210,0.04)_0%,transparent_70%)]">
          {authChecking ? (
            <div className="flex h-[60vh] items-center justify-center">
              <LoadingSpinner label="Validating session..." />
            </div>
          ) : (
            <ErrorBoundary fallbackTitle="DASHBOARD UI ERROR">
              {children}
            </ErrorBoundary>
          )}
        </main>
      </div>

      <ReactMissionModal 
        isOpen={isMissionModalOpen} 
        onClose={() => setIsMissionModalOpen(false)} 
        onSuccess={handleMissionSuccess} 
      />
    </div>
  );
}
