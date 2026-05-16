/** Structured Scout 2.0 operator report (matches backend scout_report.rs). */

export type ScoutFindingView = {
  id: string;
  source_id: string;
  source_label: string;
  title: string;
  severity: string;
  summary: string;
  url?: string;
  cves: string[];
  mitre_techniques: string[];
  tags: string[];
  ioc_count: number;
};

export type ScoutReactionView = {
  threat_id: string;
  title: string;
  severity: string;
  source: string;
  disposition: string;
  policy_note: string;
};

export type ScoutAutonomyPolicy = {
  heal_apply_enforced: boolean;
  description_ru: string;
};

export type ScoutEnrichmentView = {
  merged_duplicates: number;
  total_iocs: number;
  total_cves: number;
  total_ips: number;
  total_domains: number;
  total_hashes: number;
};

export type ScoutOperatorReport = {
  executive_summary_ru: string;
  top_findings: ScoutFindingView[];
  reactions: ScoutReactionView[];
  enrichment: ScoutEnrichmentView;
  autonomy: ScoutAutonomyPolicy;
};

export type ScoutSourceStatus = {
  id: string;
  label: string;
  status: string;
  count: number;
  note?: string;
};

export function dispositionLabel(disposition: string): string {
  switch (disposition) {
    case 'hitl_queue':
      return 'HITL очередь';
    case 'auto_apply_eligible':
      return 'Авто после sandbox';
    case 'scheduled_background':
      return 'Фоновый heal';
    default:
      return disposition;
  }
}

export function dispositionColor(disposition: string): string {
  if (disposition === 'hitl_queue') return 'text-[#fabc4e] border-[#fabc4e]/40 bg-[#fabc4e]/10';
  if (disposition === 'auto_apply_eligible') return 'text-[#00F5A3] border-[#00F5A3]/40 bg-[#00F5A3]/10';
  return 'text-white/70 border-white/20 bg-white/5';
}

export function severityClass(severity: string): string {
  if (severity === 'critical') return 'text-[#ffb4ab]';
  if (severity === 'high') return 'text-[#fabc4e]';
  return 'text-white/70';
}
