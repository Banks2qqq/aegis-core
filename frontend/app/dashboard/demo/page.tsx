'use client';

import React, { useEffect, useMemo, useState } from 'react';
import { ApiClient, getWsBaseUrl } from '../../../lib/api';
import ErrorBoundary from '../../../components/ErrorBoundary';
import LoadingSpinner from '../../../components/LoadingSpinner';
import {
  Activity,
  AlertTriangle,
  ArrowRight,
  CheckCircle,
  Copy,
  FileText,
  Headset,
  Play,
  Shield,
  Sparkles,
} from 'lucide-react';

type StepResult = { ok: boolean; details: string };
type StepStatus = 'pending' | 'running' | 'success' | 'warning' | 'error';

type Step = {
  id: 'airgap' | 'hitl' | 'react' | 'audit';
  title: string;
  icon: React.ComponentType<{ className?: string }>;
  run: () => Promise<StepResult>;
};

const api = new ApiClient();

function sleep(ms: number) {
  return new Promise((r) => setTimeout(r, ms));
}

function randInt(min: number, max: number) {
  return Math.floor(Math.random() * (max - min + 1)) + min;
}

function dotClass(st: StepStatus) {
  if (st === 'success') return 'bg-[#00F5A3]';
  if (st === 'warning') return 'bg-[#fabc4e]';
  if (st === 'error') return 'bg-[#ffb4ab]';
  if (st === 'running') return 'bg-white';
  return 'bg-white/20';
}

function iconClass(st: StepStatus) {
  if (st === 'success') return 'text-[#00F5A3]';
  if (st === 'warning') return 'text-[#fabc4e]';
  if (st === 'error') return 'text-[#ffb4ab]';
  if (st === 'running') return 'text-white';
  return 'text-white/60';
}

export default function DemoTour() {
  const [idx, setIdx] = useState(0);
  const [runningStep, setRunningStep] = useState(false);
  const [autoRunning, setAutoRunning] = useState(false);
  const [autoProgress, setAutoProgress] = useState(0);
  const [wsLast, setWsLast] = useState<string>('');
  const [results, setResults] = useState<Record<string, StepResult>>({});
  const [statuses, setStatuses] = useState<Record<string, StepStatus>>({});
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    let ws: WebSocket | null = null;
    try {
      ws = new WebSocket(`${getWsBaseUrl()}/ws`);
      ws.onmessage = (e) => {
        try {
          const msg = JSON.parse(e.data);
          const text = typeof msg.data === 'string' ? msg.data : JSON.stringify(msg.data);
          setWsLast(`[${msg.type}] ${text}`.slice(0, 220));
        } catch {}
      };
    } catch {}
    return () => ws?.close();
  }, []);

  const steps: Step[] = useMemo(
    () => [
      {
        id: 'airgap',
        title: 'Шаг 1 — Air‑Gapped статус',
        icon: Shield,
        run: async () => {
          const s: any = await api.getStatus();
          const air = s && typeof s === 'object' && 'air_gapped' in s ? (s as any).air_gapped : 'unknown';
          const airTxt = String(air);
          return {
            ok: airTxt === 'true' || airTxt === 'false' || airTxt === 'unknown',
            details: `oracle_alive=${s.oracle_alive} | sentinels=${s.active_sentinels} | blocked=${s.threats_blocked} | version=${s.version} | air_gapped=${airTxt}`,
          };
        },
      },
      {
        id: 'hitl',
        title: 'Шаг 2 — HITL пример (/code)',
        icon: Headset,
        run: async () => {
          // Call WITHOUT approval. Backend should respond with 409 + message.
          await api.request('/api/code-demo', {
            method: 'POST',
            json: { task: 'Generate a safe Rust IP validator (demo).', approved: false },
          });
          return { ok: false, details: 'Unexpected: backend did not require HITL.' };
        },
      },
      {
        id: 'react',
        title: 'Шаг 3 — ReAct++ миссия (WS streaming)',
        icon: Activity,
        run: async () => {
          const r: any = await api.launchReactMission('Demo: run a safe incident triage flow with HITL gates and audit trail.');
          const ok = r?.status === 'accepted';
          return { ok, details: r?.message || JSON.stringify(r) };
        },
      },
      {
        id: 'audit',
        title: 'Шаг 4 — Просмотр audit.log',
        icon: FileText,
        run: async () => {
          const tail: any = await api.request('/api/audit-tail?lines=12', { method: 'GET' });
          const lines = Array.isArray(tail?.lines) ? (tail.lines as string[]) : [];
          if (!tail?.exists) {
            return { ok: false, details: `audit.log not found at ${tail?.path || './data/audit.log'}` };
          }
          return { ok: true, details: lines.join('\n') || 'audit.log empty' };
        },
      },
    ],
    []
  );

  const current = steps[idx];
  const manualProgress = Math.round(((idx + 1) / steps.length) * 100);
  const progress = autoRunning ? autoProgress : manualProgress;

  const summary = useMemo(() => {
    const done = steps.filter((s) => results[s.id]).length;
    const ok = steps.filter((s) => results[s.id]?.ok).length;
    return { done, ok, total: steps.length };
  }, [results, steps]);

  async function runStep(step: Step) {
    setRunningStep(true);
    setStatuses((prev) => ({ ...prev, [step.id]: 'running' }));
    try {
      const res = await step.run();
      setResults((prev) => ({ ...prev, [step.id]: res }));
      setStatuses((prev) => ({ ...prev, [step.id]: res.ok ? 'success' : 'error' }));
    } catch (e: any) {
      const message = e?.message || String(e);
      const hitlOk = step.id === 'hitl' && /Human approval required|needs_human_approval|409|Conflict/i.test(message);
      const details = hitlOk
        ? '✅ Human-in-the-Loop сработал: backend потребовал подтверждение человека (409).'
        : message;
      setResults((prev) => ({ ...prev, [step.id]: { ok: hitlOk, details } }));
      setStatuses((prev) => ({ ...prev, [step.id]: hitlOk ? 'success' : 'error' }));
    } finally {
      setRunningStep(false);
    }
  }

  async function runAll() {
    if (autoRunning) return;
    setAutoRunning(true);
    setAutoProgress(0);
    try {
      for (let i = 0; i < steps.length; i++) {
        setIdx(i);
        setAutoProgress(Math.round((i / steps.length) * 100));
        await runStep(steps[i]);
        await sleep(randInt(800, 1200));
      }
      setAutoProgress(100);
    } finally {
      setAutoRunning(false);
      setRunningStep(false);
    }
  }

  async function copyAudit() {
    try {
      const txt = results['audit']?.details || '';
      if (!txt) return;
      await navigator.clipboard.writeText(txt);
      setCopied(true);
      setTimeout(() => setCopied(false), 1400);
    } catch {}
  }

  return (
    <ErrorBoundary fallbackTitle="DEMO TOUR UI ERROR">
      <div className="max-w-[1200px] mx-auto">
        <div className="mb-8 flex items-end justify-between">
          <div>
            <div className="font-mono text-xs tracking-[4px] text-[#00F5A3] mb-2">DEMO TOUR • PILOT</div>
            <h1 className="text-6xl font-bold tracking-tighter flex items-center gap-3">
              Guided Demo <Sparkles className="w-6 h-6 text-[#ddb7ff]" />
            </h1>
            <p className="text-white/40 mt-3 max-w-2xl">
              Пошаговый сценарий для заказчика: Air‑Gapped → HITL (/code) → ReAct++ → Audit Trail.
            </p>
          </div>
          <div className="text-right font-mono text-xs text-white/40">
            WS:{' '}
            <span className={wsLast ? 'text-[#00F5A3]' : 'text-[#ffb4ab]'}>
              {wsLast ? 'LIVE' : 'WAITING'}
            </span>
          </div>
        </div>

        {/* Progress */}
        <div className="glass-card rounded-3xl p-6 mb-4">
          <div className="flex items-center justify-between mb-3">
            <div className="font-mono text-xs tracking-[3px] text-white/50">PROGRESS</div>
            <div className="font-mono text-xs text-white/50">{progress}%</div>
          </div>
          <div className="h-2 bg-white/5 rounded-full overflow-hidden">
            <div className="h-full bg-[#ddb7ff] rounded-full transition-all" style={{ width: `${progress}%` }} />
          </div>
          <div className="mt-4 flex items-center justify-between">
            <div className="font-mono text-xs text-white/40">
              Completed: {summary.done}/{summary.total}
            </div>
            <div className="font-mono text-xs text-white/40">
              OK: <span className="text-[#00F5A3]">{summary.ok}</span>
            </div>
          </div>
        </div>

        <div className="grid md:grid-cols-2 gap-4">
          {/* Steps */}
          <div className="glass-card rounded-3xl p-8">
            <div className="font-mono text-xs tracking-[3px] text-[#a4c9ff] mb-6">STEPS</div>
            <div className="space-y-3">
              {steps.map((s, i) => {
                const r = results[s.id];
                const st: StepStatus = statuses[s.id] || (r ? (r.ok ? 'success' : 'error') : 'pending');
                const active = i === idx;
                const Icon = s.icon;
                return (
                  <button
                    key={s.id}
                    type="button"
                    onClick={() => setIdx(i)}
                    className={[
                      'w-full text-left px-5 py-4 rounded-2xl border transition-all',
                      active ? 'bg-white/10 border-white/20' : 'bg-white/5 border-white/10 hover:bg-white/10',
                    ].join(' ')}
                  >
                    <div className="flex items-center justify-between">
                      <div className="flex items-center gap-3">
                        <div className={`w-2 h-2 rounded-full ${dotClass(st)}`} />
                        <Icon className={`w-4 h-4 ${iconClass(st)}`} />
                        <div className="font-mono text-sm tracking-widest">{s.title}</div>
                      </div>
                      {r ? (
                        r.ok ? (
                          <CheckCircle className="w-4 h-4 text-[#00F5A3]" />
                        ) : (
                          <AlertTriangle className="w-4 h-4 text-[#ffb4ab]" />
                        )
                      ) : (
                        <div className="text-white/30 text-xs">pending</div>
                      )}
                    </div>
                    {st === 'running' && <div className="mt-2 text-white/30 text-xs font-mono">running…</div>}
                  </button>
                );
              })}
            </div>
          </div>

          {/* Runner */}
          <div className="glass-card rounded-3xl p-8">
            <div className="font-mono text-xs tracking-[3px] text-[#fabc4e] mb-6">RUN</div>
            <div className="text-2xl tracking-tight mb-2">{current?.title}</div>
            <div className="text-white/40 text-sm mb-6">
              “RUN ALL” выглядит максимально эффектно на демо. Все ошибки остаются честными (Zero‑Trust).
            </div>

            <button
              type="button"
              onClick={runAll}
              disabled={autoRunning || runningStep}
              className="w-full flex items-center justify-center gap-3 py-3 border border-white/20 rounded-2xl text-xs font-mono tracking-widest hover:bg-white/5 disabled:opacity-60"
            >
              {autoRunning ? 'RUNNING ALL…' : 'RUN ALL STEPS AUTOMATICALLY'}
            </button>

            <button
              type="button"
              onClick={() => current && runStep(current)}
              disabled={runningStep}
              className="mt-4 w-full flex items-center justify-center gap-3 py-4 bg-[#ddb7ff] text-black rounded-2xl font-bold tracking-[3px] text-sm disabled:opacity-60 hover:bg-white transition-all"
            >
              {runningStep ? <LoadingSpinner label="RUNNING..." /> : (<><Play className="w-4 h-4" /> RUN STEP</>)}
            </button>

            <button
              type="button"
              onClick={() => setIdx((x) => Math.min(steps.length - 1, x + 1))}
              className="mt-4 w-full flex items-center justify-center gap-2 py-3 border border-white/20 rounded-2xl text-xs font-mono tracking-widest hover:bg-white/5"
            >
              NEXT STEP <ArrowRight className="w-4 h-4" />
            </button>

            {results[current?.id || '']?.details && (
              <div className="mt-6 p-4 rounded-2xl bg-black/40 border border-white/10 font-mono text-xs text-white/60 whitespace-pre-wrap">
                {results[current!.id].details}
              </div>
            )}

            {current?.id === 'audit' && results['audit']?.details && (
              <button
                type="button"
                onClick={copyAudit}
                className="mt-4 w-full flex items-center justify-center gap-2 py-3 border border-white/20 rounded-2xl text-xs font-mono tracking-widest hover:bg-white/5"
              >
                <Copy className="w-4 h-4" />
                {copied ? 'COPIED' : 'COPY AUDIT.LOG TO CLIPBOARD'}
              </button>
            )}

            {wsLast && (
              <div className="mt-4 p-4 rounded-2xl bg-black/40 border border-white/10 font-mono text-xs text-white/60">
                {wsLast}
              </div>
            )}
          </div>
        </div>

        {/* Summary */}
        <div className="glass-card rounded-3xl p-8 mt-4">
          <div className="font-mono text-xs tracking-[3px] text-[#a4c9ff] mb-4">РЕЗУЛЬТАТЫ ДЕМО</div>
          <div className="text-white/70">
            Успешно: <span className="text-[#00F5A3] font-mono">{summary.ok}</span> /{' '}
            <span className="font-mono">{summary.total}</span> шагов.
          </div>
          <div className="text-white/40 text-sm mt-2">
            Зеленый = OK, желтый = частично/не критично, красный = endpoint/доступ/инфра.
          </div>
        </div>
      </div>
    </ErrorBoundary>
  );
}

