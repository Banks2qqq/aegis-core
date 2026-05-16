'use client';

import React, { useCallback, useEffect, useState } from 'react';
import { motion } from 'framer-motion';
import { Database, ExternalLink, RefreshCw, Radar } from 'lucide-react';
import { ApiClient, type ScoutBduItem } from '../../../lib/api';
import { loadLastScoutRun, saveLastScoutRun } from '../../../lib/scoutStorage';
import { formatScoutIngestLine } from '../../../lib/scoutSummary';
import LoadingSpinner from '../../../components/LoadingSpinner';

const api = new ApiClient();

function severityBadge(severity: string) {
  if (severity === 'critical') {
    return 'bg-[#ffb4ab]/15 text-[#ffb4ab] border-[#ffb4ab]/40';
  }
  if (severity === 'high') {
    return 'bg-[#fabc4e]/15 text-[#fabc4e] border-[#fabc4e]/40';
  }
  return 'bg-white/10 text-white/60 border-white/20';
}

export default function BduThreatIntelPage() {
  const [items, setItems] = useState<ScoutBduItem[]>([]);
  const [lastScout, setLastScout] = useState<{
    completed_at?: number;
    found?: number;
    ingested?: number;
    ingested_new?: number;
    ingested_updated?: number;
    fusion_updated?: number;
    deception_deployed?: number;
    healing_attempted?: number;
    healing_applied?: number;
    status?: string;
  } | null>(null);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState('');

  const loadRecent = useCallback(async () => {
    try {
      setError('');
      const data = await api.getBduRecent();
      setItems(data.items || []);
      if (data.last_scout) setLastScout(data.last_scout);
    } catch (e: unknown) {
      const stored = loadLastScoutRun();
      if (stored?.items?.length) {
        setItems(stored.items);
        setLastScout({
          completed_at: Date.parse(stored.completed_at),
          found: stored.found,
          ingested: stored.ingested,
          status: stored.status,
        });
      }
      setError((e as Error)?.message || 'Не удалось загрузить список BDU');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadRecent();
    const onScout = () => loadRecent();
    window.addEventListener('aegis_scout_complete', onScout);
    return () => window.removeEventListener('aegis_scout_complete', onScout);
  }, [loadRecent]);

  const refreshFromBdu = async () => {
    setRefreshing(true);
    setError('');
    try {
      const res = await api.runScout();
      if (res.status === 'success') {
        saveLastScoutRun(res);
        await loadRecent();
      } else {
        setError(res.error || 'Ошибка Scout');
      }
    } catch (e: unknown) {
      setError((e as Error)?.message || 'Scout failed');
    } finally {
      setRefreshing(false);
    }
  };

  if (loading) {
    return (
      <div className="flex h-96 items-center justify-center">
        <LoadingSpinner label="Загрузка BDU..." />
      </div>
    );
  }

  return (
    <div className="space-y-8">
      <motion.div
        initial={{ opacity: 0, y: 8 }}
        animate={{ opacity: 1, y: 0 }}
        className="flex flex-col md:flex-row md:items-end md:justify-between gap-4"
      >
        <div>
          <div className="font-mono text-xs tracking-[4px] text-[#00F5A3] mb-2 flex items-center gap-2">
            <Database className="w-4 h-4" />
            FSTEC BDU
          </div>
          <h1 className="text-4xl font-bold tracking-tight">BDU Threat Intel</h1>
          <p className="text-white/60 mt-2 max-w-xl">
            Уязвимости из официальной базы ФСТЭК (bdu.fstec.ru): критический и высокий уровень опасности.
          </p>
        </div>
        <button
          type="button"
          onClick={refreshFromBdu}
          disabled={refreshing}
          className="inline-flex items-center gap-2 px-5 py-2.5 rounded-xl border border-[#00F5A3]/40 text-[#00F5A3] font-mono text-xs tracking-widest hover:bg-[#00F5A3]/10 disabled:opacity-50 transition-all"
        >
          <RefreshCw className={`w-4 h-4 ${refreshing ? 'animate-spin' : ''}`} />
          {refreshing ? 'ОБНОВЛЕНИЕ...' : 'ОБНОВИТЬ ИЗ БДУ'}
        </button>
      </motion.div>

      {lastScout?.completed_at && (
        <div className="glass-card rounded-2xl p-4 border border-white/10 flex items-center gap-4 text-sm font-mono">
          <Radar className="w-5 h-5 text-[#00F5A3]" />
          <span className="text-white/50">Последний Scout:</span>
          <span className="text-[#00F5A3]">{lastScout.found ?? 0} найдено</span>
          <span className="text-white/30">·</span>
          <span className="text-[#ddb7ff]">{formatScoutIngestLine(lastScout)}</span>
          {lastScout.fusion_updated != null && (
            <>
              <span className="text-white/30">·</span>
              <span className="text-[#fabc4e]">Fusion {lastScout.fusion_updated}</span>
            </>
          )}
          {lastScout.deception_deployed != null && (
            <>
              <span className="text-white/30">·</span>
              <span className="text-[#00F5A3]">Honeypots {lastScout.deception_deployed}</span>
            </>
          )}
          {lastScout.healing_applied != null && (
            <>
              <span className="text-white/30">·</span>
              <span className="text-[#ff6b9d]">
                Self-Heal {lastScout.healing_applied}/{lastScout.healing_attempted ?? 0}
              </span>
            </>
          )}
          <span className="text-white/30">·</span>
          <span className="text-white/40">
            {new Date(lastScout.completed_at * 1000).toLocaleString('ru-RU')}
          </span>
        </div>
      )}

      {error && (
        <motion.div className="text-[#ffb4ab] text-sm bg-[#ffb4ab]/10 border border-[#ffb4ab]/30 rounded-xl px-4 py-3">
          {error}
        </motion.div>
      )}

      <div className="glass-card rounded-3xl overflow-hidden border border-white/10">
        <div className="px-6 py-4 border-b border-white/10 flex justify-between items-center">
          <span className="font-mono text-xs text-white/50 tracking-widest">
            ПОСЛЕДНИЕ {items.length} ЗАПИСЕЙ
          </span>
        </div>
        <div className="divide-y divide-white/5">
          {items.length === 0 ? (
            <div className="p-12 text-center text-white/40">
              Нет данных. Нажмите «Обновить из БДУ» или SCOUT в шапке.
            </div>
          ) : (
            items.map((item, i) => (
              <motion.div
                key={item.id}
                initial={{ opacity: 0, x: -8 }}
                animate={{ opacity: 1, x: 0 }}
                transition={{ delay: i * 0.03 }}
                className="px-6 py-4 hover:bg-white/[0.02] flex flex-col md:flex-row md:items-center gap-3 md:gap-6"
              >
                <div className="flex items-center gap-3 md:w-48 shrink-0">
                  <span
                    className={`text-[10px] font-mono uppercase px-2 py-1 rounded border ${severityBadge(item.severity)}`}
                  >
                    {item.severity}
                  </span>
                  <a
                    href={item.url}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="font-mono text-sm text-[#00F5A3] hover:underline flex items-center gap-1"
                  >
                    {item.bdu_id}
                    <ExternalLink className="w-3 h-3 opacity-50" />
                  </a>
                </div>
                <p className="flex-1 text-sm text-white/80 line-clamp-2">{item.title}</p>
                {item.published && (
                  <span className="text-xs font-mono text-white/40 shrink-0">{item.published}</span>
                )}
              </motion.div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
