'use client';

import React, { useEffect, useState } from 'react';
import { motion } from 'framer-motion';
import Link from 'next/link';
import { 
  Shield, 
  Users, 
  Target, 
  Zap, 
  Activity, 
  RefreshCw,
  Radar,
  ExternalLink
} from 'lucide-react';
import { ApiClient, type KnowledgeResponse, type StatusResponse } from '../../../lib/api';
import { useAegisWebSocket } from '../../../lib/useAegisWebSocket';
import { loadLastScoutRun, type StoredScoutRun } from '../../../lib/scoutStorage';
import { formatScoutIngestLine } from '../../../lib/scoutSummary';

const api = new ApiClient();

function formatLiveEvent(text: string): { kind: 'scout' | 'heal' | 'react' | 'contain' | 'other'; body: string } {
  if (text.includes('[SCOUT')) return { kind: 'scout', body: text };
  if (text.includes('[SELF-HEALING]')) return { kind: 'heal', body: text };
  if (text.includes('[ReAct++]')) return { kind: 'react', body: text };
  if (text.includes('[CONTAIN]')) return { kind: 'contain', body: text };
  return { kind: 'other', body: text };
}

function severityClass(severity: string) {
  if (severity === 'critical') return 'text-[#ffb4ab]';
  if (severity === 'high') return 'text-[#fabc4e]';
  return 'text-white/70';
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

      {/* Последний Scout */}
      <div className="glass-card rounded-3xl p-6 border border-[#00F5A3]/20">
        <div className="flex items-center justify-between mb-4">
          <h3 className="text-lg font-semibold flex items-center gap-2">
            <Radar className="w-5 h-5 text-[#00F5A3]" />
            Последний запуск Scout (ФСТЭК БДУ)
          </h3>
          <Link
            href="/dashboard/bdu"
            className="text-xs font-mono text-[#00F5A3] hover:underline tracking-widest"
          >
            BDU Threat Intel →
          </Link>
        </div>
        {lastScout && lastScout.status === 'success' ? (
          <div className="space-y-4">
            <div className="flex flex-wrap gap-4 text-sm font-mono">
              <span className="text-[#00F5A3]">
                Найдено <strong className="text-xl">{lastScout.found}</strong> уязвимостей
              </span>
              <span className="text-white/30">·</span>
              <span className="text-[#ddb7ff]">{formatScoutIngestLine(lastScout)}</span>
              <span className="text-white/30">·</span>
              <span className="text-white/50">
                {new Date(lastScout.completed_at).toLocaleString('ru-RU')}
              </span>
            </div>
            <motion.div className="flex flex-wrap gap-3" initial={{ opacity: 0 }} animate={{ opacity: 1 }}>
              {lastScout.ingested_new != null && (
                <PipelineChip label="Новых в KB" value={lastScout.ingested_new} color="#ddb7ff" />
              )}
              {lastScout.ingested_updated != null && (
                <PipelineChip label="Обновлено" value={lastScout.ingested_updated} color="#a4c9ff" />
              )}
              {lastScout.fusion_updated != null && (
                <PipelineChip label="Fusion" value={lastScout.fusion_updated} color="#fabc4e" />
              )}
              {lastScout.deception_deployed != null && (
                <PipelineChip label="Honeypots" value={lastScout.deception_deployed} color="#00F5A3" />
              )}
              {lastScout.healing_applied != null && (
                <PipelineChip label="Self-Heal" value={lastScout.healing_applied} color="#ff6b9d" />
              )}
              {lastScout.critic_verdict && (
                <span className="text-xs font-mono px-3 py-1.5 rounded-lg border border-white/10 text-white/50">
                  Critic: {lastScout.critic_verdict} ({(lastScout.critic_risk ?? 0).toFixed(2)})
                </span>
              )}
            </motion.div>
            <div className="grid gap-2 md:grid-cols-3">
              {lastScout.items.slice(0, 3).map((item) => (
                <a
                  key={item.id}
                  href={item.url}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="p-3 rounded-xl bg-black/40 border border-white/10 hover:border-[#00F5A3]/30 transition-colors group"
                >
                  <div className="flex items-center justify-between gap-2 mb-1">
                    <span className={`font-mono text-xs uppercase ${severityClass(item.severity)}`}>
                      {item.severity}
                    </span>
                    <ExternalLink className="w-3 h-3 text-white/30 group-hover:text-[#00F5A3]" />
                  </div>
                  <div className="font-mono text-xs text-[#00F5A3]">{item.bdu_id}</div>
                  <div className="text-xs text-white/50 line-clamp-2 mt-1">{item.title}</div>
                </a>
              ))}
            </div>
          </div>
        ) : (
          <p className="text-white/50 text-sm">
            Scout ещё не запускался. Нажмите <strong className="text-[#00F5A3]">SCOUT</strong> в шапке.
          </p>
        )}
      </div>

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

function PipelineChip({
  label,
  value,
  color,
}: {
  label: string;
  value: number;
  color: string;
}) {
  return (
    <div
      className="flex items-center gap-2 px-3 py-1.5 rounded-lg border border-white/10 bg-black/30 text-xs font-mono"
      style={{ borderColor: `${color}33` }}
    >
      <span className="text-white/50">{label}</span>
      <strong className="text-lg" style={{ color }}>{value}</strong>
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
