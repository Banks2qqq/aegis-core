/** Cross-page dashboard events (PR2). */

export type ContainResultDetail = {
  cluster_id: string;
  isolation_level?: string;
  runtime?: string;
  network?: string;
  threats_blocked?: number;
};

export function dispatchContainComplete(detail: ContainResultDetail) {
  if (typeof window === 'undefined') return;
  window.dispatchEvent(new CustomEvent('aegis_contain_complete', { detail }));
}

export function dispatchStatusRefresh() {
  if (typeof window === 'undefined') return;
  window.dispatchEvent(new CustomEvent('aegis_status_refresh'));
}

export function dispatchOpenReactModal() {
  if (typeof window === 'undefined') return;
  window.dispatchEvent(new CustomEvent('aegis_open_react_modal'));
}

export function dispatchReactMissionStarted(mission: string) {
  if (typeof window === 'undefined') return;
  window.dispatchEvent(new CustomEvent('aegis_react_started', { detail: { mission } }));
}
