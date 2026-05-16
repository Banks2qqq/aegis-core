'use client';

import React from 'react';
import Link from 'next/link';
import { ExternalLink, Radar, Shield } from 'lucide-react';
import type { ScoutResult } from '../lib/api';
import {
  dispositionColor,
  dispositionLabel,
  severityClass,
  type ScoutOperatorReport,
  type ScoutSourceStatus,
} from '../lib/scoutReport';
import { formatScoutIngestLine } from '../lib/scoutSummary';

type ScoutRun = Pick<
  ScoutResult,
  | 'found'
  | 'ingested_new'
  | 'ingested_updated'
  | 'sources_ok'
  | 'sources_skipped'
  | 'sources_failed'
  | 'fusion_updated'
  | 'deception_deployed'
  | 'healing_attempted'
  | 'critic_verdict'
  | 'critic_risk'
> & {
  completed_at?: string | number;
  sources?: ScoutSourceStatus[];
  report?: ScoutOperatorReport | null;
};

export default function ScoutReportPanel({ run }: { run: ScoutRun }) {
  if (!run || run.found === 0) return null;

  const report = run.report;
  const sources = (run.sources ?? []) as ScoutSourceStatus[];

  return (
    <div className="glass-card rounded-3xl p-6 border border-[#00F5A3]/25 space-y-6">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div>
          <div className="font-mono text-[10px] tracking-[3px] text-[#00F5A3] mb-1">SCOUT 2.0 — ОТЧЁТ</div>
          <h3 className="text-lg font-semibold flex items-center gap-2">
            <Radar className="w-5 h-5 text-[#00F5A3]" />
            Структурированный результат цикла
          </h3>
        </div>
        <Link
          href="/dashboard/healing"
          className="text-xs font-mono text-[#ff6b9d] hover:underline tracking-widest"
        >
          HITL / Healing →
        </Link>
      </div>

      {report?.executive_summary_ru && (
        <p className="text-sm text-white/80 leading-relaxed border-l-2 border-[#00F5A3]/50 pl-4">
          {report.executive_summary_ru}
        </p>
      )}

      <div className="flex flex-wrap gap-3 text-xs font-mono">
        <Chip label="Находок" value={String(run.found)} />
        <Chip label="Источников OK" value={String(run.sources_ok ?? '—')} />
        <Chip label="Ingest" value={formatScoutIngestLine(run)} accent="green" />
        <Chip label="Fusion" value={String(run.fusion_updated ?? '—')} />
        <Chip label="Honeypots" value={String(run.deception_deployed ?? '—')} />
        <Chip label="Heal queued" value={String(run.healing_attempted ?? 0)} accent="pink" />
        {run.critic_verdict && (
          <span className="px-3 py-1.5 rounded-lg border border-white/10 text-white/50">
            Critic: {run.critic_verdict} ({(run.critic_risk ?? 0).toFixed(2)})
          </span>
        )}
      </div>

      {report?.autonomy && (
        <div className="rounded-2xl border border-white/10 bg-black/30 p-4 flex gap-3">
          <Shield className="w-5 h-5 text-[#ddb7ff] shrink-0 mt-0.5" />
          <div>
            <div className="text-xs font-mono text-[#ddb7ff] tracking-widest mb-1">ПОЛИТИКА АВТОНОМИИ</div>
            <p className="text-sm text-white/70">{report.autonomy.description_ru}</p>
          </div>
        </div>
      )}

      {report?.enrichment && (
        <div>
          <div className="text-xs font-mono text-white/40 tracking-widest mb-2">ОБОГАЩЕНИЕ</div>
          <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-6 gap-2 text-center">
            {(
              [
                ['IOC', report.enrichment.total_iocs],
                ['CVE', report.enrichment.total_cves],
                ['IP', report.enrichment.total_ips],
                ['Домены', report.enrichment.total_domains],
                ['Хэши', report.enrichment.total_hashes],
                ['Дедуп', report.enrichment.merged_duplicates],
              ] as const
            ).map(([k, v]) => (
              <div key={k} className="rounded-xl bg-black/40 border border-white/10 py-2 px-1">
                <div className="text-[10px] text-white/40">{k}</div>
                <div className="text-lg font-bold text-[#a4c9ff]">{v}</div>
              </div>
            ))}
          </div>
        </div>
      )}

      {sources.length > 0 && (
        <div>
          <div className="text-xs font-mono text-white/40 tracking-widest mb-2">ИСТОЧНИКИ</div>
          <div className="flex flex-wrap gap-2">
            {sources.map((s) => (
              <span
                key={s.id}
                className={`text-[10px] font-mono px-2 py-1 rounded-lg border ${
                  s.status === 'ok'
                    ? 'border-[#00F5A3]/30 text-[#00F5A3]/90'
                    : s.status === 'skipped'
                      ? 'border-white/15 text-white/40'
                      : 'border-[#ffb4ab]/30 text-[#ffb4ab]/90'
                }`}
                title={s.note}
              >
                {s.label}: {s.count}
              </span>
            ))}
          </div>
        </div>
      )}

      {report?.reactions && report.reactions.length > 0 && (
        <div>
          <div className="text-xs font-mono text-white/40 tracking-widest mb-2">АВТОРЕАКЦИЯ (запланировано)</div>
          <ul className="space-y-2">
            {report.reactions.map((r) => (
              <li key={r.threat_id} className="p-3 rounded-xl bg-black/40 border border-white/10 text-sm">
                <div className="flex flex-wrap items-center gap-2 mb-1">
                  <span className={`text-xs uppercase font-mono ${severityClass(r.severity)}`}>
                    {r.severity}
                  </span>
                  <span
                    className={`text-[10px] font-mono uppercase px-2 py-0.5 rounded border ${dispositionColor(r.disposition)}`}
                  >
                    {dispositionLabel(r.disposition)}
                  </span>
                  <span className="text-[10px] text-white/40 font-mono">{r.source}</span>
                </div>
                <div className="text-white/80 line-clamp-2">{r.title}</div>
                <p className="text-xs text-white/45 mt-1">{r.policy_note}</p>
              </li>
            ))}
          </ul>
        </div>
      )}

      {report?.top_findings && report.top_findings.length > 0 && (
        <div>
          <div className="text-xs font-mono text-white/40 tracking-widest mb-2">TOP УГРОЗ (все источники)</div>
          <div className="grid gap-2 md:grid-cols-2">
            {report.top_findings.slice(0, 8).map((f) => (
              <div
                key={f.id}
                className="p-3 rounded-xl bg-black/40 border border-white/10 hover:border-[#00F5A3]/20 transition-colors"
              >
                <div className="flex items-center justify-between gap-2 mb-1">
                  <span className={`text-[10px] uppercase font-mono ${severityClass(f.severity)}`}>
                    {f.severity}
                  </span>
                  <span className="text-[10px] text-white/40 truncate">{f.source_label}</span>
                </div>
                <div className="text-sm text-white/90 line-clamp-2">{f.title}</div>
                {(f.cves.length > 0 || f.mitre_techniques.length > 0) && (
                  <div className="text-[10px] font-mono text-white/45 mt-2 space-y-0.5">
                    {f.cves.length > 0 && <div>CVE: {f.cves.slice(0, 3).join(', ')}</div>}
                    {f.mitre_techniques.length > 0 && (
                      <div>MITRE: {f.mitre_techniques.slice(0, 3).join(', ')}</div>
                    )}
                  </div>
                )}
                {f.url && (
                  <a
                    href={f.url}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="inline-flex items-center gap-1 text-[10px] text-[#00F5A3] mt-2 hover:underline"
                  >
                    источник <ExternalLink className="w-3 h-3" />
                  </a>
                )}
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

function Chip({
  label,
  value,
  accent,
}: {
  label: string;
  value: string;
  accent?: 'green' | 'pink';
}) {
  const color =
    accent === 'pink' ? 'text-[#ff6b9d]' : accent ? 'text-[#ddb7ff]' : 'text-white/80';
  return (
    <span className="px-3 py-1.5 rounded-lg border border-white/10 bg-black/30">
      <span className="text-white/40">{label}: </span>
      <span className={color}>{value}</span>
    </span>
  );
}
