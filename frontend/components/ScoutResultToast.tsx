'use client';

import React from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { X, ExternalLink, Radar } from 'lucide-react';
import type { StoredScoutRun } from '../lib/scoutStorage';
import { formatScoutIngestLine } from '../lib/scoutSummary';

function severityColor(severity: string) {
  if (severity === 'critical') return 'text-[#ffb4ab] border-[#ffb4ab]/40 bg-[#ffb4ab]/10';
  if (severity === 'high') return 'text-[#fabc4e] border-[#fabc4e]/40 bg-[#fabc4e]/10';
  return 'text-white/70 border-white/20 bg-white/5';
}

export default function ScoutResultToast({
  result,
  onClose,
}: {
  result: StoredScoutRun | null;
  onClose: () => void;
}) {
  if (!result || result.status !== 'success') return null;

  const hasIngestBreakdown =
    result.ingested_new != null && result.ingested_updated != null;
  const totalFindings = result.total_findings ?? result.found;
  const sourcesOk = result.sources_ok;

  return (
    <AnimatePresence>
      <motion.div
        key="scout-toast"
        initial={{ opacity: 0, y: -16, scale: 0.96 }}
        animate={{ opacity: 1, y: 0, scale: 1 }}
        exit={{ opacity: 0, y: -12, scale: 0.96 }}
        className="fixed top-20 right-8 z-[200] w-full max-w-md"
      >
        <motion.div
          className="glass-card rounded-2xl border border-[#00F5A3]/30 bg-[#0a0a0f]/95 shadow-[0_0_40px_rgba(0,245,163,0.12)] p-5"
          role="status"
        >
          <motion.div className="flex items-start justify-between gap-3 mb-4">
            <div className="flex items-center gap-3">
              <motion.div className="w-10 h-10 rounded-xl bg-[#00F5A3]/15 flex items-center justify-center">
                <Radar className="w-5 h-5 text-[#00F5A3]" />
              </motion.div>
              <motion.div>
                <motion.div className="font-mono text-[10px] tracking-[3px] text-[#00F5A3]">
                  SCOUT 2.0
                </motion.div>
                <motion.div className="font-semibold text-white">
                  Автономный цикл завершён
                </motion.div>
              </motion.div>
            </div>
            <button
              type="button"
              onClick={onClose}
              className="p-1 rounded-lg hover:bg-white/10 text-white/50"
              aria-label="Закрыть"
            >
              <X className="w-4 h-4" />
            </button>
          </motion.div>

          <motion.div
            className={`grid gap-3 mb-4 ${hasIngestBreakdown ? 'grid-cols-2 sm:grid-cols-3' : 'grid-cols-2'}`}
          >
            <motion.div className="rounded-xl bg-black/40 border border-white/10 px-3 py-3">
              <motion.div className="text-xs text-white/50">Записей</motion.div>
              <motion.div className="text-2xl font-bold text-[#00F5A3]">{totalFindings}</motion.div>
              {sourcesOk != null && (
                <motion.div className="text-[10px] text-white/40 mt-1">
                  источников OK: {sourcesOk}
                </motion.div>
              )}
            </motion.div>
            {hasIngestBreakdown && (
              <>
                <motion.div className="rounded-xl bg-black/40 border border-white/10 px-3 py-3">
                  <motion.div className="text-xs text-white/50">Новых</motion.div>
                  <motion.div className="text-2xl font-bold text-[#ddb7ff]">
                    {result.ingested_new}
                  </motion.div>
                </motion.div>
                <motion.div className="rounded-xl bg-black/40 border border-white/10 px-3 py-3">
                  <motion.div className="text-xs text-white/50">Обновлено</motion.div>
                  <motion.div className="text-2xl font-bold text-[#a4c9ff]">
                    {result.ingested_updated}
                  </motion.div>
                </motion.div>
              </>
            )}
            <motion.div className="rounded-xl bg-black/40 border border-white/10 px-3 py-3">
              <motion.div className="text-xs text-white/50">Fusion / Traps</motion.div>
              <motion.div className="text-sm font-bold text-[#fabc4e] leading-tight mt-1">
                {result.fusion_updated ?? '—'} / {result.deception_deployed ?? '—'}
              </motion.div>
            </motion.div>
            {(result.healing_attempted != null && result.healing_attempted > 0) && (
              <motion.div className="rounded-xl bg-black/40 border border-[#ff6b9d]/30 px-3 py-3">
                <motion.div className="text-xs text-white/50">Self-Heal (фон)</motion.div>
                <motion.div className="text-sm font-bold text-[#ff6b9d] leading-tight mt-1">
                  в очереди: {result.healing_attempted}
                </motion.div>
              </motion.div>
            )}
          </motion.div>

          <p className="text-xs text-[#ddb7ff]/90 font-mono mb-2">{formatScoutIngestLine(result)}</p>
          {result.report?.executive_summary_ru && (
            <p className="text-xs text-white/55 line-clamp-3 mb-2">{result.report.executive_summary_ru}</p>
          )}

          <motion.div className="text-xs text-white/40 font-mono mb-2 space-y-1">
            <motion.div>{new Date(result.completed_at).toLocaleString('ru-RU')}</motion.div>
            {result.critic_verdict && (
              <motion.div>
                Critic: {result.critic_verdict} (risk {(result.critic_risk ?? 0).toFixed(2)})
              </motion.div>
            )}
            {(result.sources_failed ?? 0) > 0 && (
              <motion.div className="text-[#fabc4e]">
                источников с ошибкой: {result.sources_failed}
              </motion.div>
            )}
          </motion.div>

          <motion.div className="space-y-2">
            {result.items.slice(0, 3).map((item) => (
              <a
                key={item.id}
                href={item.url}
                target="_blank"
                rel="noopener noreferrer"
                className="flex items-start gap-2 p-2 rounded-lg bg-black/30 border border-white/5 hover:border-[#00F5A3]/30 transition-colors group"
              >
                <span
                  className={`shrink-0 text-[10px] font-mono uppercase px-2 py-0.5 rounded border ${severityColor(item.severity)}`}
                >
                  {item.severity}
                </span>
                <motion.div className="min-w-0 flex-1">
                  <motion.div className="font-mono text-xs text-[#00F5A3] group-hover:underline">
                    {item.bdu_id}
                  </motion.div>
                  <motion.div className="text-xs text-white/60 truncate">{item.title}</motion.div>
                </motion.div>
                <ExternalLink className="w-3.5 h-3.5 shrink-0 text-white/30 group-hover:text-[#00F5A3]" />
              </a>
            ))}
          </motion.div>
        </motion.div>
      </motion.div>
    </AnimatePresence>
  );
}
