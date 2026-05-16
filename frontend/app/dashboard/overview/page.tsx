'use client';

import React, { useEffect, useState } from 'react';
import { motion } from 'framer-motion';
import { 
  Shield, 
  Users, 
  Target, 
  Zap, 
  Activity, 
  RefreshCw,
  Radar,
} from 'lucide-react';
import { ApiClient, type KnowledgeResponse, type StatusResponse } from '../../../lib/api';
import { useAegisWebSocket } from '../../../lib/useAegisWebSocket';
import { loadLastScoutRun, type StoredScoutRun } from '../../../lib/scoutStorage';
import ScoutReportPanel from '../../../components/ScoutReportPanel';

const api = new ApiClient();

function formatLiveEvent(text: string): { kind: 'scout' | 'heal' | 'react' | 'contain' | 'other'; body: string } {
  if (text.includes('[SCOUT')) return { kind: 'scout', body: text };
  if (text.includes('[SELF-HEALING]')) return { kind: 'heal', body: text };
  if (text.includes('[ReAct++]')) return { kind: 'react', body: text };
  if (text.includes('[CONTAIN]')) return { kind: 'contain', body: text };
  return { kind: 'other', body: text };
}

export default function OverviewWarRoom() {
  const [status, setStatus] = useState<StatusResponse | null>(null);
  const [knowledge, setKnowledge] = useState<KnowledgeResponse | null>(null);
  const [liveEvents, setLiveEvents] = useState<string[]>([]);
  const [lastScout, setLastScout] = useState<StoredScoutRun | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);

  useEffect(() => {
    const loadStatus = async () => {
      try {
        setLoadError(null);
        const [statusData, knowledgeData] = await Promise.all([
          api.getStatus(),
          api.getKnowledge().catch(() => null),
        ]);
        setStatus(statusData);
        setKnowledge(knowledgeData);
      } catch (error) {
        console.error('Failed to load status:', error);
        setLoadError(
          error instanceof Error ? error.message : 'Не удалось связаться с API (проверьте сеть и JWT)'
        );
      } finally {
        setLoading(false);
      }
    };
    loadStatus();
    setLastScout(loadLastScoutRun());

    const onScout = (e: Event) => {
      const detail = (e as CustomEvent<StoredScoutRun>).detail;
      if (detail) setLastScout(detail);
    };
    const refreshStatus = async () => {
      try {
        const data = await api.getStatus();
        setStatus(data);
      } catch {
        /* ignore */
      }
    };

    window.addEventListener('aegis_scout_complete', onScout);
    window.addEventListener('aegis_status_refresh', refreshStatus);
    window.addEventListener('aegis_contain_complete', refreshStatus);
    return () => {
      window.removeEventListener('aegis_scout_complete', onScout);
      window.removeEventListener('aegis_status_refresh', refreshStatus);
      window.removeEventListener('aegis_contain_complete', refreshStatus);
    };
  }, []);

  useAegisWebSocket((msg: any) => {
    if (msg?.type === 'alert' || msg?.type === 'init') {
      const text = typeof msg.data === 'string' ? msg.data : JSON.stringify(msg.data);
      setLiveEvents((prev) => [text, ...prev].slice(0, 12));
    }
  });

  if (loading) {
    return (
      <div className="flex items-center justify-center h-96">
        <RefreshCw className="w-8 h-8 animate-spin text-[#00F5A3]" />
      </div>
    );
  }

  return (
    <div className="space-y-8">
      <div>
        <motion.div className="font-mono text-xs tracking-[4px] text-[#00F5A3] mb-2">WAR ROOM</motion.div>
        <h1 className="text-4xl font-bold tracking-tight">Overview</h1>
        <p className="text-white/60 mt-2">Состояние автономной защиты в реальном времени</p>
      </div>

      {loadError && (
        <div
          className="rounded-2xl border border-amber-500/40 bg-amber-500/10 px-4 py-3 text-sm text-amber-100"
          role="alert"
        >
          <span className="font-semibold text-amber-200">API недоступен. </span>
          {loadError}
        </div>
      )}

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
        <StatusCard
          icon={<Shield className="w-6 h-6" />}
          title="Oracle"
          value={status?.oracle_alive ? 'Online' : 'Offline'}
          color={status?.oracle_alive ? '#00F5A3' : '#ffb4ab'}
        />
        <StatusCard
          icon={<Users className="w-6 h-6" />}
          title="Sentinels"
          value={String(status?.active_sentinels ?? 0)}
          color="#3B82F6"
        />
        <StatusCard
          icon={<Target className="w-6 h-6" />}
          title="Threats blocked"
          value={String(status?.threats_blocked ?? 0)}
          color="#8B5CF6"
        />
        <StatusCard
          icon={<Zap className="w-6 h-6" />}
          title="Shield"
          value={status?.shield_active ? 'Active' : 'Standby'}
          color="#F59E0B"
        />
      </div>
      <p className="text-xs text-white/40 font-mono">
        v{status?.version || '—'} · BDU KB {status?.bdu_kb_count ?? 0} · Black KB {status?.black_kb_count ?? 0}
        · Fusion {status?.fusion_clusters ?? 0}
        {status?.air_gapped ? ' · AIR-GAPPED' : ''}
      </p>

      {knowledge && (knowledge.bdu?.length > 0 || knowledge.other_intel?.length > 0) && (
        <div className="glass-card rounded-3xl p-6 border border-white/10">
          <h3 className="text-lg font-semibold mb-4">Black Knowledge (live)</h3>
          <div className="grid gap-4 md:grid-cols-2">
            <div>
              <div className="text-xs font-mono text-[#00F5A3] mb-2 tracking-widest">ФСТЭК BDU</div>
              <ul className="space-y-2 text-sm text-white/70 font-mono">
                {(knowledge.bdu || []).slice(0, 5).map((line, i) => (
                  <li key={i} className="line-clamp-2">{line}</li>
                ))}
              </ul>
            </div>
            {knowledge.other_intel?.length > 0 && (
              <div>
                <div className="text-xs font-mono text-[#ddb7ff] mb-2 tracking-widest">OTHER INTEL</div>
                <ul className="space-y-2 text-sm text-white/70 font-mono">
                  {knowledge.other_intel.slice(0, 4).map((line, i) => (
                    <li key={i} className="line-clamp-2">{line}</li>
                  ))}
                </ul>
              </div>
            )}
          </div>
        </div>
      )}

      {lastScout && lastScout.status === 'success' ? (
        <ScoutReportPanel run={lastScout} />
      ) : (
        <div className="glass-card rounded-3xl p-6 border border-[#00F5A3]/20">
          <h3 className="text-lg font-semibold flex items-center gap-2 mb-2">
            <Radar className="w-5 h-5 text-[#00F5A3]" />
            Scout 2.0
          </h3>
          <p className="text-white/50 text-sm">
            Scout ещё не запускался. Нажмите <strong className="text-[#00F5A3]">SCOUT</strong> в шапке.
          </p>
        </div>
      )}

      <div className="glass-card rounded-3xl p-8">
        <div className="flex items-center justify-between mb-6">
          <h3 className="text-xl font-semibold flex items-center gap-3">
            <Activity className="w-5 h-5 text-[#00F5A3]" />
            Live Events
          </h3>
          <div className="text-xs text-white/50">Последние 12 событий</div>
        </div>

        <div className="space-y-3 font-mono text-sm">
          {liveEvents.length > 0 ? (
            liveEvents.map((event, index) => {
              const { kind, body } = formatLiveEvent(event);
              return (
                <div
                  key={index}
                  className={`p-3 rounded-xl border ${
                    kind === 'scout'
                      ? 'bg-[#00F5A3]/5 border-[#00F5A3]/25 text-[#c8ffe8]'
                      : kind === 'heal'
                        ? 'bg-[#ff6b9d]/5 border-[#ff6b9d]/25 text-[#ffd0e0]'
                        : kind === 'react'
                          ? 'bg-[#ddb7ff]/5 border-[#ddb7ff]/25 text-[#ead4ff]'
                          : kind === 'contain'
                            ? 'bg-[#ffb4ab]/5 border-[#ffb4ab]/25 text-[#ffe0dc]'
                            : 'bg-black/40 border-white/10'
                  }`}
                >
                  {body}
                </div>
              );
            })
          ) : (
            <div className="text-white/50 py-8 text-center">Ожидание событий...</div>
          )}
        </div>
      </div>
    </div>
  );
}

function StatusCard({ 
  icon, 
  title, 
  value, 
  color 
}: { 
  icon: React.ReactNode; 
  title: string; 
  value: string; 
  color: string;
}) {
  return (
    <motion.div 
      whileHover={{ scale: 1.02 }}
      className="glass-card rounded-3xl p-6 border-l-4"
      style={{ borderLeftColor: color }}
    >
      <div className="flex items-start justify-between">
        <div>
          <div className="text-white/60 text-sm mb-1">{title}</div>
          <div className="text-3xl font-bold tracking-tight">{value}</div>
        </div>
        <div style={{ color }} className="mt-1">
          {icon}
        </div>
      </div>
    </motion.div>
  );
}
