'use client';

import React, { useEffect, useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { AlertTriangle, Shield, Zap, Clock, Target } from 'lucide-react';
import { ApiClient, type ContainResult, type FusedThreatRow } from '../../../lib/api';
import { dispatchContainComplete, dispatchStatusRefresh } from '../../../lib/aegisEvents';
import { useToast } from '../../../components/Toast';
import { useAegisWebSocket } from '../../../lib/useAegisWebSocket';
import ErrorBoundary from '../../../components/ErrorBoundary';
import LoadingSpinner from '../../../components/LoadingSpinner';

type IocField = string | { ioc_type?: string; value?: string; type?: string };

type FusedThreat = FusedThreatRow & { iocs: IocField[] };

function formatIoc(ioc: IocField): string {
  if (typeof ioc === 'string') return ioc;
  const t = ioc.ioc_type || ioc.type || 'ioc';
  const v = ioc.value || '';
  return v ? `${t}:${v}` : t;
}

function formatTs(ts: string | number): string {
  if (typeof ts === 'number') {
    const ms = ts > 1_000_000_000_000 ? ts : ts * 1000;
    return new Date(ms).toLocaleString('ru-RU');
  }
  const d = new Date(ts);
  return Number.isNaN(d.getTime()) ? String(ts) : d.toLocaleString('ru-RU');
}

const api = new ApiClient();

function exportThreats(threats: FusedThreat[], format: 'json' | 'csv') {
  if (threats.length === 0) return alert('No data to export');

  if (format === 'json') {
    const blob = new Blob([JSON.stringify(threats, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `aegis-threats-${new Date().toISOString().slice(0,10)}.json`;
    a.click();
    URL.revokeObjectURL(url);
  } else {
    const headers = ['cluster_id', 'severity', 'confidence', 'sources', 'iocs', 'summary', 'first_seen', 'last_seen'];
    const rows = threats.map(t => [
      t.cluster_id,
      t.severity,
      t.confidence,
      t.sources.join('|'),
      t.iocs.map(formatIoc).join('|'),
      `"${t.summary.replace(/"/g, '""')}"`,
      t.first_seen,
      t.last_seen,
    ]);
    const csv = [headers.join(','), ...rows.map(r => r.join(','))].join('\n');
    const blob = new Blob([csv], { type: 'text/csv' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `aegis-threats-${new Date().toISOString().slice(0,10)}.csv`;
    a.click();
    URL.revokeObjectURL(url);
  }
}

export default function ThreatIntelligence() {
  const { showToast } = useToast();
  const [threats, setThreats] = useState<FusedThreat[]>([]);
  const [loading, setLoading] = useState(true);
  const [lastUpdate, setLastUpdate] = useState<Date>(new Date());
  const [error, setError] = useState<string>('');
  const [containingId, setContainingId] = useState<string | null>(null);

  // Fetch fused threats
  const fetchThreats = async () => {
    try {
      setError('');
      const data: FusedThreat[] = await api.getFusedThreats();
      setThreats(data);
      setLastUpdate(new Date());
    } catch (e) {
      console.error('Failed to fetch threats:', e);
      setError((e as any)?.message || 'Failed to fetch threats');
    } finally {
      setLoading(false);
    }
  };

  const wsStatus = useAegisWebSocket((msg: any) => {
    if (msg?.type === 'alert' || msg?.type === 'threat') {
      fetchThreats();
    }
  });

  // Initial load + polling fallback
  useEffect(() => {
    let mounted = true;
    setTimeout(() => {
      if (mounted) fetchThreats();
    }, 0);
    const interval = setInterval(() => { if (mounted) fetchThreats(); }, 15000); // fallback poll every 15s
    return () => {
      mounted = false;
      clearInterval(interval);
    };
  }, []);

  const getSeverityColor = (sev: number) => {
    if (sev >= 0.85) return '#ffb4ab';
    if (sev >= 0.6) return '#fabc4e';
    return '#a4c9ff';
  };

  return (
    <ErrorBoundary fallbackTitle="THREATS UI ERROR">
      {loading && threats.length === 0 ? (
        <div className="flex h-[60vh] items-center justify-center">
          <LoadingSpinner label="Loading fused threats..." />
        </div>
      ) : (
    <div className="max-w-[1600px] mx-auto">
      {/* Header */}
      <div className="flex items-end justify-between mb-10">
        <div>
          <div className="font-mono text-xs tracking-[4px] text-[#a4c9ff] mb-2 flex items-center gap-2">
            THREAT INTELLIGENCE ENGINE
            <div className={`w-1.5 h-1.5 rounded-full ${wsStatus === 'connected' ? 'bg-[#00F5A3]' : 'bg-[#ffb4ab] animate-pulse'}`} />
          </div>
          <h1 className="text-4xl font-bold tracking-tight">Threat Intelligence</h1>
        </div>
        <div className="flex items-center gap-4">
          <div className="text-right font-mono text-xs text-white/40">
            LAST SYNC: {lastUpdate.toLocaleTimeString('ru-RU')}<br />
            <span className="text-[#ddb7ff]">{threats.length} FUSED CLUSTERS</span>
          </div>
        <div className="flex gap-2">
          <button onClick={() => exportThreats(threats, 'json')} className="px-4 py-2 text-xs border border-white/20 rounded-xl hover:bg-white/5 font-mono tracking-widest transition-colors active:scale-[0.985]">JSON</button>
          <button onClick={() => exportThreats(threats, 'csv')} className="px-4 py-2 text-xs border border-white/20 rounded-xl hover:bg-white/5 font-mono tracking-widest transition-colors active:scale-[0.985]">CSV</button>
        </div>
        </div>
      </div>

      {loading && threats.length === 0 ? (
        <div className="flex items-center justify-center h-64 text-white/40">Loading threat clusters...</div>
      ) : (
        <div className="space-y-4">
          <AnimatePresence>
            {error && (
              <div className="glass-card rounded-3xl p-10 border border-[#ffb4ab]/30">
                <div className="flex items-center gap-3 text-[#ffb4ab] font-mono tracking-widest text-xs">
                  <AlertTriangle className="w-4 h-4" />
                  {error}
                </div>
              </div>
            )}
            {threats.length === 0 && (
              <div className="glass-card rounded-3xl p-16 text-center">
                <Shield className="w-12 h-12 mx-auto mb-6 text-white/30" />
                <div className="text-2xl tracking-tight">No active fused threats</div>
                <div className="text-white/40 mt-2">The system is currently clear. New clusters will appear here in real-time.</div>
              </div>
            )}

            {threats.map((threat, index) => (
              <motion.div
                key={threat.cluster_id}
                initial={{ opacity: 0, y: 20 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0 }}
                transition={{ delay: index * 0.03 }}
                className="glass-card rounded-3xl p-8 border-l-4"
                style={{ borderLeftColor: getSeverityColor(threat.severity) }}
              >
                <div className="flex flex-col lg:flex-row lg:items-start gap-8">
                  {/* Severity & Meta */}
                  <div className="lg:w-48 flex-shrink-0">
                    <div className="flex items-center gap-3 mb-4">
                      <AlertTriangle className="w-5 h-5" style={{ color: getSeverityColor(threat.severity) }} />
                      <div>
                        <div className="font-mono text-[10px] tracking-[2px] text-white/40">SEVERITY</div>
                        <div className="text-5xl font-bold tabular-nums tracking-tighter" style={{ color: getSeverityColor(threat.severity) }}>
                          {(threat.severity * 100).toFixed(0)}
                        </div>
                      </div>
                    </div>
                    <div className="text-xs font-mono text-white/40">
                      CONFIDENCE: <span className="text-white">{(threat.confidence * 100).toFixed(0)}%</span>
                    </div>
                  </div>

                  {/* Content */}
                  <div className="flex-1 min-w-0">
                    <div className="font-mono text-xs tracking-[3px] text-[#a4c9ff] mb-2">
                      CLUSTER {threat.cluster_id.slice(0, 8).toUpperCase()}
                    </div>
                    <div className="text-xl leading-tight tracking-tight mb-4 pr-8">
                      {threat.summary}
                    </div>

                    {/* Sources */}
                    <div className="flex flex-wrap gap-2 mb-5">
                      {threat.sources.map((src, i) => (
                        <div key={i} className="px-3 py-1 rounded-full bg-white/5 text-xs font-mono tracking-widest border border-white/10">
                          {src}
                        </div>
                      ))}
                    </div>

                    {/* IOCs */}
                    {threat.iocs.length > 0 && (
                      <div className="mb-5">
                        <div className="text-xs font-mono tracking-widest text-white/40 mb-2">INDICATORS OF COMPROMISE</div>
                        <div className="flex flex-wrap gap-x-6 gap-y-1 text-sm font-mono text-white/70">
                          {threat.iocs.slice(0, 6).map((ioc, i) => (
                            <span key={i} className="hover:text-[#ddb7ff] cursor-pointer transition-colors">{formatIoc(ioc)}</span>
                          ))}
                          {threat.iocs.length > 6 && <span className="text-white/30">+{threat.iocs.length - 6} more</span>}
                        </div>
                      </div>
                    )}

                    {/* Time */}
                    <div className="flex items-center gap-6 text-xs font-mono text-white/40">
                      <div className="flex items-center gap-1.5">
                        <Clock className="w-3.5 h-3.5" /> FIRST SEEN: {formatTs(threat.first_seen)}
                      </div>
                      <div>LAST SEEN: {formatTs(threat.last_seen)}</div>
                    </div>
                  </div>

                  {/* Action */}
                  <div className="lg:pt-2">
                    <div className="flex flex-col gap-2">
                      <button
                        type="button"
                        disabled={threat.contained || containingId === threat.cluster_id}
                        onClick={async () => {
                          setContainingId(threat.cluster_id);
                          try {
                            const res: ContainResult = await api.containCluster(threat.cluster_id);
                            if (res.status === 'contained') {
                              setThreats((prev) =>
                                prev.map((t) =>
                                  t.cluster_id === threat.cluster_id ? { ...t, contained: true } : t
                                )
                              );
                              dispatchContainComplete({
                                cluster_id: res.cluster_id,
                                isolation_level: res.isolation_level,
                                runtime: res.runtime,
                                network: res.network,
                                threats_blocked: res.threats_blocked,
                              });
                              dispatchStatusRefresh();
                              showToast(
                                `Contain ${res.enforcement_mode ?? 'policy'} · ${res.isolation_level}/${res.runtime} · blocked=${res.threats_blocked ?? '—'}`
                              );
                            } else {
                              showToast(res.message || 'Contain failed');
                            }
                          } catch (e: unknown) {
                            showToast((e as Error)?.message || 'Contain API error');
                          } finally {
                            setContainingId(null);
                          }
                        }}
                        className="px-8 py-3 bg-[#ffb4ab] text-black rounded-2xl text-xs font-bold tracking-[2px] hover:bg-white transition-all active:scale-[0.985] whitespace-nowrap disabled:opacity-40 disabled:cursor-not-allowed"
                      >
                        {threat.contained
                          ? 'CONTAINED'
                          : containingId === threat.cluster_id
                            ? 'ISOLATING…'
                            : 'CONTAIN CLUSTER'}
                      </button>
                      <button
                        type="button"
                        onClick={() => exportThreats([threat], 'json')}
                        className="px-8 py-2 border border-white/20 rounded-xl text-xs font-mono tracking-widest hover:bg-white/5"
                      >
                        EXPORT JSON
                      </button>
                    </div>
                  </div>
                </div>
              </motion.div>
            ))}
          </AnimatePresence>
        </div>
      )}
    </div>
      )}
    </ErrorBoundary>
  );
}
