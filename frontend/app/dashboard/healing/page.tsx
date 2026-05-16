'use client';

import React, { useCallback, useEffect, useState } from 'react';
import { HeartPulse, CheckCircle2, XCircle, RefreshCw, Play } from 'lucide-react';
import { ApiClient, PendingHealPatch } from '../../../lib/api';
import { useToast } from '../../../components/Toast';

const api = new ApiClient();

export default function HealingPage() {
  const { showToast } = useToast();
  const [pending, setPending] = useState<PendingHealPatch[]>([]);
  const [loading, setLoading] = useState(false);
  const [actionId, setActionId] = useState<string | null>(null);
  const [anomaly, setAnomaly] = useState('');
  const [patchType, setPatchType] = useState('code');

  const loadPending = useCallback(async () => {
    try {
      const res = await api.getHealPending();
      setPending(res.items ?? []);
    } catch {
      setPending([]);
    }
  }, []);

  useEffect(() => {
    loadPending();
    const t = setInterval(loadPending, 20_000);
    return () => clearInterval(t);
  }, [loadPending]);

  const handleApprove = async (patchId: string) => {
    if (!confirm(`Применить патч ${patchId} на диск?`)) return;
    setActionId(patchId);
    setLoading(true);
    try {
      const res = await api.approveHeal(patchId);
      showToast(`Патч применён: ${res.path ?? patchId}`, 'success');
      await loadPending();
    } catch (e) {
      showToast(e instanceof Error ? e.message : 'Ошибка approve', 'error');
    } finally {
      setLoading(false);
      setActionId(null);
    }
  };

  const handleReject = async (patchId: string) => {
    const reason = prompt('Причина отклонения:') ?? 'operator-reject';
    setActionId(patchId);
    setLoading(true);
    try {
      await api.rejectHeal(patchId, reason);
      showToast('Патч отклонён', 'success');
      await loadPending();
    } catch (e) {
      showToast(e instanceof Error ? e.message : 'Ошибка reject', 'error');
    } finally {
      setLoading(false);
      setActionId(null);
    }
  };

  const handleRun = async () => {
    const text = anomaly.trim();
    if (!text) {
      showToast('Укажите описание аномалии', 'error');
      return;
    }
    setLoading(true);
    try {
      const res = await api.runHeal(text, patchType);
      if (res.pending_hitl) {
        showToast('Цикл завершён: патч в очереди HITL', 'success');
      } else if (res.result?.applied) {
        showToast('Патч применён автоматически (низкий риск)', 'success');
      } else {
        showToast(`Цикл: ${res.result?.audit_event ?? 'завершён'}`, 'success');
      }
      setAnomaly('');
      await loadPending();
    } catch (e) {
      showToast(e instanceof Error ? e.message : 'Ошибка heal/run', 'error');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="max-w-[1100px] mx-auto">
      <div className="mb-10">
        <div className="font-mono text-xs tracking-[4px] text-[#ff6b9d] mb-2">SELF-HEALING • HITL</div>
        <h1 className="text-3xl font-bold tracking-tight flex items-center gap-3">
          <HeartPulse className="w-8 h-8 text-[#ff6b9d]" />
          Healing &amp; HITL
        </h1>
        <p className="text-white/50 mt-3 text-sm max-w-2xl">
          Полный цикл: Inquisitor → formal verify → Docker sandbox → очередь HITL (High/Critical) или apply по политике.
          Scout при критических BDU ставит патчи в эту же очередь.
        </p>
      </div>

      <div className="glass-card rounded-3xl p-8 border border-white/10 mb-8">
        <div className="font-mono text-xs tracking-widest text-white/60 mb-4">ЗАПУСК ЦИКЛА</div>
        <textarea
          value={anomaly}
          onChange={(e) => setAnomaly(e.target.value)}
          placeholder="Описание аномалии (как в Scout / CLI /heal)"
          className="w-full min-h-[100px] rounded-2xl bg-black/30 border border-white/10 p-4 text-sm font-mono text-white/80 resize-y"
        />
        <div className="flex flex-wrap items-center gap-4 mt-4">
          <label className="font-mono text-xs text-white/50">
            patch_type
            <select
              value={patchType}
              onChange={(e) => setPatchType(e.target.value)}
              className="ml-2 bg-black/40 border border-white/15 rounded-lg px-3 py-2 text-white"
            >
              <option value="config">config (low)</option>
              <option value="code">code (high → HITL)</option>
              <option value="custom">custom (critical → HITL)</option>
            </select>
          </label>
          <button
            onClick={handleRun}
            disabled={loading}
            className="flex items-center gap-2 px-5 py-2.5 rounded-xl bg-[#ff6b9d]/20 text-[#ff6b9d] border border-[#ff6b9d]/40 hover:bg-[#ff6b9d]/30 disabled:opacity-40 font-mono text-xs tracking-widest"
          >
            <Play className="w-4 h-4" />
            RUN HEAL
          </button>
        </div>
      </div>

      <div className="glass-card rounded-3xl p-8 border border-[#ff6b9d]/25">
        <div className="flex items-center justify-between mb-6">
          <div className="font-mono text-sm tracking-widest">ОЧЕРЕДЬ HITL</div>
          <button
            onClick={() => loadPending()}
            disabled={loading}
            className="flex items-center gap-2 text-xs font-mono px-4 py-2 border border-white/20 rounded-xl hover:bg-white/5"
          >
            <RefreshCw className={`w-3 h-3 ${loading ? 'animate-spin' : ''}`} />
            REFRESH
          </button>
        </div>
        {pending.length === 0 ? (
          <p className="text-white/40 text-sm font-mono text-center py-8 border border-dashed border-white/10 rounded-2xl">
            Нет ожидающих патчей. Появятся после Scout (critical/high) или RUN HEAL с code/custom.
          </p>
        ) : (
          <ul className="space-y-4">
            {pending.map((item) => (
              <li key={item.patch_id} className="rounded-2xl border border-white/10 bg-black/20 p-5">
                <div className="flex flex-wrap justify-between gap-2 mb-2 font-mono text-xs">
                  <span className="text-[#ff6b9d]">{item.patch_id}</span>
                  <span className="text-white/50">
                    {item.risk} · severity {item.verification_severity.toFixed(2)} · sandbox{' '}
                    {item.sandbox_passed ? 'ok' : 'fail'}
                  </span>
                </div>
                <p className="text-white/45 text-sm mb-1">{item.anomaly_summary}</p>
                <p className="text-white/30 text-xs font-mono mb-4">
                  {new Date(item.queued_at * 1000).toLocaleString()}
                </p>
                <div className="flex gap-3">
                  <button
                    onClick={() => handleApprove(item.patch_id)}
                    disabled={loading}
                    className="flex items-center gap-2 px-4 py-2 rounded-xl bg-[#00F5A3]/15 text-[#00F5A3] border border-[#00F5A3]/40 disabled:opacity-40 text-xs font-mono"
                  >
                    <CheckCircle2 className="w-4 h-4" />
                    {actionId === item.patch_id && loading ? '…' : 'APPROVE'}
                  </button>
                  <button
                    onClick={() => handleReject(item.patch_id)}
                    disabled={loading}
                    className="flex items-center gap-2 px-4 py-2 rounded-xl bg-[#ffb4ab]/10 text-[#ffb4ab] border border-[#ffb4ab]/30 disabled:opacity-40 text-xs font-mono"
                  >
                    <XCircle className="w-4 h-4" />
                    REJECT
                  </button>
                </div>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}

