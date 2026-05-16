'use client';

import React, { useEffect, useState } from 'react';
import { Users, RefreshCw, CheckCircle, AlertTriangle, Activity } from 'lucide-react';
import {
  ApiClient,
  FederationHealthReport,
  FederationNode,
  FederationOpsMetrics,
  RaftMetrics,
  RaftStatus,
} from '../../../lib/api';
import { useToast } from '../../../components/Toast';

const api = new ApiClient();

export default function FederationPage() {
  const { showToast } = useToast();
  const [nodes, setNodes] = useState<FederationNode[]>([]);
  const [health, setHealth] = useState<FederationHealthReport | null>(null);
  const [opsMetrics, setOpsMetrics] = useState<FederationOpsMetrics | null>(null);
  const [raftStatus, setRaftStatus] = useState<RaftStatus | null>(null);
  const [raftMetrics, setRaftMetrics] = useState<RaftMetrics | null>(null);
  const [loading, setLoading] = useState(true);
  const [syncing, setSyncing] = useState<string | null>(null);

  const loadFederationData = async () => {
    try {
      const [nodesData, healthData, fedMetrics, raftData, metricsData] = await Promise.all([
        api.getFederationNodes(),
        api.getFederationHealth(),
        api.getFederationMetrics().catch(() => null),
        api.getRaftStatus(),
        api.getRaftMetrics(),
      ]);
      setNodes(nodesData || []);
      setHealth(healthData?.report ?? null);
      setOpsMetrics(fedMetrics);
      setRaftStatus(raftData ?? healthData?.raft ?? null);
      setRaftMetrics(metricsData ?? null);
    } catch (error) {
      console.error('Failed to load federation data:', error);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    let mounted = true;
    setTimeout(() => {
      if (mounted) {
        loadFederationData();
      }
    }, 0);
    return () => {
      mounted = false;
    };
  }, []);

  const handleSync = async (node: FederationNode) => {
    if (!node.url || node.url === '(local)' || node.role === 'primary') {
      showToast('Локальная нода — sync не требуется');
      return;
    }
    setSyncing(node.id);
    try {
      const res = await api.syncFederationNode({ peerId: node.id, peerUrl: node.url });
      if (res?.success && res.result) {
        const merkle = res.result.merkle_match ? 'merkle ✓' : 'merkle ≠';
        showToast(`Sync ${node.id}: ${res.result.synced} записей (${merkle})`);
      } else if (res?.result?.error) {
        showToast(`Sync ${node.id}: ${res.result.error}`);
      } else {
        showToast(`Синхронизация с ${node.id} завершена`);
      }
      await loadFederationData();
    } catch {
      showToast('Ошибка синхронизации');
    } finally {
      setSyncing(null);
    }
  };

  const handleSyncAll = async () => {
    const remote = nodes.filter((n) => n.url !== '(local)' && n.role !== 'primary');
    if (remote.length === 0) {
      showToast('Нет удалённых peers в config.yaml');
      return;
    }
    setSyncing('__all__');
    try {
      const res = await api.syncFederationNode({ syncAll: true });
      const ok = res?.results?.filter((r) => r.success).length ?? 0;
      const total = res?.results?.length ?? 0;
      const matched = res?.results?.filter((r) => r.merkle_match).length ?? 0;
      showToast(`Sync all: ${ok}/${total} peers, merkle match ${matched}/${total}`);
      await loadFederationData();
    } catch {
      showToast('Ошибка sync all');
    } finally {
      setSyncing(null);
    }
  };

  const activeRaft = Number(raftStatus?.active_nodes ?? 0);
  const raftNodes = raftStatus?.nodes ?? [];

  const raftStatusColor = (status: string) => {
    if (status === 'live') return 'bg-[#00F5A3]/20 text-[#00F5A3]';
    if (status === 'candidate') return 'bg-amber-500/20 text-amber-300';
    return 'bg-red-500/20 text-red-300';
  };

  const peerStatusColor = (status?: string) => {
    if (status === 'online') return 'bg-[#00F5A3]/20 text-[#00F5A3]';
    if (status === 'degraded') return 'bg-amber-500/20 text-amber-300';
    return 'bg-red-500/20 text-red-300';
  };

  if (loading) {
    return (
      <div className="flex justify-center py-20">
        <RefreshCw className="w-8 h-8 animate-spin" />
      </div>
    );
  }

  return (
    <div className="space-y-8">
      <div>
        <div className="font-mono text-xs tracking-[4px] text-[#00F5A3] mb-2">NETWORK</div>
        <h1 className="text-4xl font-bold tracking-tight">Federation</h1>
        <p className="text-white/60 mt-2">Federation sync, health-check и Raft ops plane</p>
      </div>

      {opsMetrics && opsMetrics.peers.length > 0 && (
        <div className="glass-card rounded-3xl p-8">
          <h3 className="text-xl font-semibold mb-4">Federation Ops</h3>
          <p className="text-xs text-white/50 mb-4">
            Метрики также в Prometheus: <code className="text-white/70">/metrics</code> (
            <code>aegis_federation_*</code>)
          </p>
          <div className="grid gap-3 md:grid-cols-2">
            {opsMetrics.peers.map((p) => (
              <div key={p.id} className="p-4 rounded-2xl bg-black/40 border border-white/10">
                <div className="flex items-center justify-between gap-2">
                  <span className="font-mono text-sm">{p.id}</span>
                  <span className={`text-xs px-2 py-0.5 rounded-full ${peerStatusColor(p.status)}`}>
                    {p.status}
                  </span>
                </div>
                <div className="text-[10px] font-mono text-white/45 mt-2 break-all">
                  Public: {p.url}
                </div>
                <div className="text-[10px] font-mono text-[#00F5A3]/70 break-all">
                  Federation: {p.federation_url}
                </div>
                <div className="text-xs text-white/50 mt-2 flex flex-wrap gap-3">
                  {p.latency_ms != null && <span>probe {p.latency_ms} ms</span>}
                  {p.last_sync_duration_ms != null && (
                    <span>last sync {p.last_sync_duration_ms} ms</span>
                  )}
                  <span>ready: {p.federation_ready ? 'yes' : 'no'}</span>
                </div>
                {p.last_error && (
                  <div className="text-[10px] text-amber-400/90 mt-1 break-words">{p.last_error}</div>
                )}
              </div>
            ))}
          </div>
        </div>
      )}

      {health && (
        <div className="glass-card rounded-3xl p-8">
          <h3 className="text-xl font-semibold mb-6 flex items-center gap-3">
            <Activity className="w-5 h-5 text-[#00F5A3]" />
            Federation Health
          </h3>
          <div className="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-6 gap-6">
            <div>
              <div className="text-white/60 text-sm">Local node</div>
              <div className="text-lg font-mono mt-1">{health.local_node_id}</div>
              {health.local_public_url && (
                <div className="text-[10px] font-mono text-white/45 mt-1 break-all">
                  Public: {health.local_public_url}
                </div>
              )}
              {health.local_federation_url && (
                <div className="text-[10px] font-mono text-[#00F5A3]/70 mt-0.5 break-all">
                  Federation: {health.local_federation_url}
                </div>
              )}
            </div>
            <div>
              <div className="text-white/60 text-sm">Local Merkle</div>
              <div className="text-xs font-mono mt-1 break-all text-white/80">{health.local_merkle}</div>
            </div>
            <div>
              <div className="text-white/60 text-sm">Peers online</div>
              <div className="text-2xl font-mono mt-1 text-[#00F5A3]">
                {health.peers_online}/{health.peer_count}
              </div>
            </div>
            <div>
              <div className="text-white/60 text-sm">Checked</div>
              <div className="text-xs font-mono mt-1 text-white/70">
                {health.checked_at
                  ? new Date(health.checked_at * 1000).toLocaleString()
                  : '—'}
              </div>
            </div>
            <div>
              <div className="text-white/60 text-sm">Peer auth</div>
              <div className="text-sm font-mono mt-1 text-[#00F5A3]">
                {health.auth_enabled ? 'token on' : 'insecure/dev'}
              </div>
            </div>
            <div>
              <div className="text-white/60 text-sm">mTLS client</div>
              <div className="text-sm font-mono mt-1">{health.mtls_enabled ? 'enabled' : 'off'}</div>
            </div>
          </div>
        </div>
      )}

      <div className="glass-card rounded-3xl p-8 space-y-8">
        <h3 className="text-xl font-semibold flex items-center gap-3">
          <Users className="w-5 h-5 text-[#00F5A3]" />
          Raft Consensus
        </h3>

        <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-6">
          <div>
            <div className="text-white/60 text-sm">Leader</div>
            <div className="text-lg font-mono mt-1 truncate">{raftStatus?.leader_id || raftStatus?.leader || '—'}</div>
          </div>
          <div>
            <div className="text-white/60 text-sm">Term</div>
            <div className="text-2xl font-mono mt-1">{raftStatus?.term ?? 0}</div>
          </div>
          <div>
            <div className="text-white/60 text-sm">Commit Index</div>
            <div className="text-2xl font-mono mt-1">{raftStatus?.commit_index ?? 0}</div>
          </div>
          <div>
            <div className="text-white/60 text-sm">Last Applied</div>
            <div className="text-2xl font-mono mt-1">{raftStatus?.last_applied ?? 0}</div>
          </div>
          <div>
            <div className="text-white/60 text-sm">Log Size</div>
            <div className="text-2xl font-mono mt-1">{raftStatus?.log_size ?? 0}</div>
          </div>
          <div>
            <div className="text-white/60 text-sm">Last Log Index</div>
            <div className="text-2xl font-mono mt-1">{raftStatus?.last_log_index ?? 0}</div>
          </div>
        </div>

        {raftMetrics && (
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4 p-4 rounded-2xl bg-black/30 border border-white/10">
            <div>
              <div className="text-white/50 text-xs">Replications</div>
              <div className="font-mono text-lg">{raftMetrics.replication_count}</div>
            </div>
            <div>
              <div className="text-white/50 text-xs">Avg commit (ms)</div>
              <div className="font-mono text-lg">{raftMetrics.avg_commit_time_ms.toFixed(1)}</div>
            </div>
            <div>
              <div className="text-white/50 text-xs">Leader elections</div>
              <div className="font-mono text-lg">{raftMetrics.election_count}</div>
            </div>
            <div>
              <div className="text-white/50 text-xs">Active / Total</div>
              <div className="font-mono text-lg text-[#00F5A3]">
                {activeRaft}/{raftStatus?.total_nodes ?? 0}
              </div>
            </div>
          </div>
        )}

        {raftNodes.length > 0 && (
          <div>
            <div className="text-white/60 text-sm mb-3">Cluster nodes</div>
            <div className="space-y-2">
              {raftNodes.map((n) => (
                <div
                  key={n.id}
                  className="flex items-center justify-between p-3 rounded-xl bg-black/40 border border-white/10"
                >
                  <div className="flex items-center gap-3 min-w-0">
                    <span className={`px-2 py-0.5 rounded text-xs font-mono ${raftStatusColor(n.status)}`}>
                      {n.status}
                    </span>
                    <span className="font-mono text-sm truncate">{n.id}</span>
                    {n.is_leader && (
                      <span className="text-[10px] uppercase tracking-wider text-[#00F5A3]">leader</span>
                    )}
                  </div>
                  <div className="text-xs text-white/50 font-mono text-right">
                    <div>{n.role}</div>
                    <div>hb {n.last_heartbeat_age_secs ?? 0}s ago</div>
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>

      <div className="glass-card rounded-3xl p-8">
        <div className="flex items-center justify-between mb-6 flex-wrap gap-4">
          <h3 className="text-xl font-semibold">Connected Nodes</h3>
          <div className="flex items-center gap-3">
            <button
              type="button"
              onClick={handleSyncAll}
              disabled={syncing !== null}
              className="flex items-center gap-2 px-4 py-2 bg-[#00F5A3]/10 border border-[#00F5A3]/30 rounded-xl text-sm text-[#00F5A3] hover:bg-[#00F5A3]/20 transition-colors disabled:opacity-40"
            >
              <RefreshCw className={`w-4 h-4 ${syncing === '__all__' ? 'animate-spin' : ''}`} />
              Sync All
            </button>
            <button
              type="button"
              onClick={loadFederationData}
              className="flex items-center gap-2 text-sm text-white/70 hover:text-white transition-colors"
            >
              <RefreshCw className="w-4 h-4" /> Обновить
            </button>
          </div>
        </div>

        <div className="space-y-4">
          {nodes.length > 0 ? (
            nodes.map((node) => {
              const isLocal = node.url === '(local)' || node.role === 'primary';
              const busy = syncing === node.id;
              return (
                <div
                  key={node.id}
                  className="flex items-center justify-between p-4 bg-black/40 rounded-2xl border border-white/10 flex-wrap gap-4"
                >
                  <div className="flex items-center gap-4 min-w-0">
                    <div
                      className={`w-3 h-3 rounded-full shrink-0 ${
                        node.online ? 'bg-[#00F5A3]' : 'bg-red-500'
                      }`}
                    />
                    <div className="min-w-0">
                      <div className="font-mono text-sm flex items-center gap-2 flex-wrap">
                        {node.id}
                        {node.status && (
                          <span
                            className={`text-[10px] px-2 py-0.5 rounded-full ${peerStatusColor(node.status)}`}
                          >
                            {node.status}
                          </span>
                        )}
                        {node.federation_ready && (
                          <CheckCircle className="w-3.5 h-3.5 text-[#00F5A3]" />
                        )}
                        {node.error && !node.online && (
                          <AlertTriangle className="w-3.5 h-3.5 text-amber-400" />
                        )}
                      </div>
                      <div className="text-xs text-white/50 space-y-0.5 mt-1">
                        <div className="truncate">
                          <span className="text-white/40">Public </span>
                          {node.url}
                        </div>
                        {node.federation_url && node.federation_url !== node.url && (
                          <div className="truncate text-[#00F5A3]/80">
                            <span className="text-white/40">Federation </span>
                            {node.federation_url}
                          </div>
                        )}
                      </div>
                      {node.latency_ms != null && (
                        <div className="text-xs text-white/40 mt-0.5">{node.latency_ms} ms</div>
                      )}
                    </div>
                  </div>

                  <div className="flex items-center gap-4 flex-wrap">
                    <div className="text-right text-sm max-w-xs">
                      <div className="text-white/70">Last sync</div>
                      <div className="font-mono text-xs break-words">{node.last_sync || '—'}</div>
                      {node.merkle_root && (
                        <div className="font-mono text-[10px] text-white/40 mt-1 truncate max-w-[200px]">
                          merkle: {node.merkle_root}
                          {node.merkle_match != null && (
                            <span className={node.merkle_match ? ' text-[#00F5A3]' : ' text-amber-400'}>
                              {' '}
                              {node.merkle_match ? '✓ match' : '≠ drift'}
                            </span>
                          )}
                        </div>
                      )}
                    </div>

                    {!isLocal && (
                      <button
                        type="button"
                        onClick={() => handleSync(node)}
                        disabled={!node.online || syncing !== null}
                        className="px-4 py-2 bg-white/10 hover:bg-white/20 rounded-xl text-sm transition-colors active:scale-[0.985] disabled:opacity-40 flex items-center gap-2"
                      >
                        {busy ? <RefreshCw className="w-4 h-4 animate-spin" /> : null}
                        Sync Now
                      </button>
                    )}
                  </div>
                </div>
              );
            })
          ) : (
            <div className="text-center py-12 text-white/50">
              Нет peers — добавьте в config.yaml → federation.peers
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
