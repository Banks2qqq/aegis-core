/** Подписи для результата SCOUT (новые vs обновлённые записи в KB). */

export type ScoutIngestFields = {
  ingested?: number;
  ingested_new?: number;
  ingested_updated?: number;
};

export function formatScoutIngestLine(run: ScoutIngestFields): string {
  const nNew = run.ingested_new;
  const nUpdated = run.ingested_updated;
  if (nNew != null && nUpdated != null) {
    if (nNew === 0 && nUpdated > 0) {
      return `${nUpdated} обновлено в Black Knowledge (новых: 0)`;
    }
    if (nNew > 0 && nUpdated === 0) {
      return `${nNew} новых в Black Knowledge`;
    }
    if (nNew > 0 && nUpdated > 0) {
      return `${nNew} новых · ${nUpdated} обновлено`;
    }
    return 'база без изменений (все записи уже были)';
  }
  const total = run.ingested ?? 0;
  return `${total} в Black Knowledge`;
}

export function formatScoutIngestShort(run: ScoutIngestFields): { newLabel: string; updatedLabel: string } {
  const nNew = run.ingested_new ?? 0;
  const nUpdated = run.ingested_updated ?? run.ingested ?? 0;
  return {
    newLabel: String(nNew),
    updatedLabel: String(run.ingested_updated != null ? nUpdated : '—'),
  };
}
