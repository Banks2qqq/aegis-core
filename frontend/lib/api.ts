export type ScoutBduItem = {
  id: string;
  bdu_id: string;
  title: string;
  severity: string;
  url: string;
  published?: string;
};

export type ContainResult = {
  status: string;
  cluster_id: string;
  severity?: number;
  isolation_level?: string;
  runtime?: string;
  network?: string;
  threats_blocked?: number;
  fusion_marked?: boolean;
  enforcement_mode?: string;
  host_enforced?: boolean;
  contain_record?: Record<string, unknown>;
  message?: string;
};

export type FusedThreatRow = {
  cluster_id: string;
  severity: number;
  confidence: number;
  sources: string[];
  iocs: Array<string | { ioc_type?: string; value?: string; type?: string }>;
  summary: string;
  first_seen: string | number;
  last_seen: string | number;
  contained?: boolean;
};

export type ScoutResult = {
  status: string;
  found: number;
  ingested: number;
  ingested_new?: number;
  ingested_updated?: number;
  source: string;
  completed_at?: number;
  items: ScoutBduItem[];
  critic_verdict?: string;
  critic_risk?: number;
  inquisitor_blocks?: number;
  inquisitor_escalates?: number;
  fusion_updated?: number;
  deception_deployed?: number;
  healing_attempted?: number;
  healing_applied?: number;
  total_findings?: number;
  sources_ok?: number;
  sources_skipped?: number;
  sources_failed?: number;
  enrichment_merged?: number;
  total_iocs?: number;
  total_cves?: number;
  pipeline?: string[];
  error?: string;
};

export type BduRecentResponse = {
  items: ScoutBduItem[];
  last_scout?: {
    completed_at: number;
    found: number;
    ingested: number;
    ingested_new?: number;
    ingested_updated?: number;
    fusion_updated?: number;
    deception_deployed?: number;
    healing_attempted?: number;
    healing_applied?: number;
    total_findings?: number;
    sources_ok?: number;
    sources_skipped?: number;
    sources_failed?: number;
    critic_verdict?: string;
    critic_risk?: number;
    status: string;
  } | null;
};

export type StatusResponse = {
  oracle_alive?: boolean;
  active_sentinels?: number;
  threats_blocked?: number;
  osint_documents?: number;
  darknet_documents?: number;
  black_kb_count?: number;
  bdu_kb_count?: number;
  fusion_clusters?: number;
  shield_active?: boolean;
  version?: string;
  air_gapped?: boolean;
  react_ready?: boolean;
  llm_ready?: boolean;
  llm_cloud_available?: boolean;
  llm_local_available?: boolean;
};

export type ReactLlmStatus = {
  react_ready: boolean;
  air_gapped: boolean;
  cloud_available: boolean;
  local_available: boolean;
  llm_ready: boolean;
  cloud_provider?: string | null;
  default_model?: string | null;
};

export type KnowledgeResponse = {
  bdu: string[];
  other_intel: string[];
  black_kb_count?: number;
  osint?: string[];
  darknet?: string[];
};

export type FederationNode = {
  id: string;
  url: string;
  federation_url?: string;
  status?: 'online' | 'degraded' | 'offline' | string;
  last_sync_duration_ms?: number;
  online?: boolean;
  health_ok?: boolean;
  federation_ready?: boolean;
  last_sync?: string;
  last_sync_at?: number;
  last_sync_count?: number;
  latency_ms?: number;
  remote_merkle?: string;
  merkle_root?: string;
  merkle_match?: boolean;
  version?: string;
  error?: string;
  role?: string;
};

export type FederationOpsMetrics = {
  checked_at: number;
  local_node_id: string;
  peers: Array<{
    id: string;
    status: string;
    url: string;
    federation_url: string;
    federation_ready: boolean;
    latency_ms?: number;
    last_sync_at?: number;
    last_sync_duration_ms?: number;
    last_error?: string;
  }>;
};

export type FederationHealthReport = {
  local_node_id: string;
  local_public_url?: string;
  local_federation_url?: string;
  local_online: boolean;
  local_merkle: string;
  peer_count: number;
  peers_online: number;
  peers: FederationNode[];
  checked_at: number;
  auth_enabled?: boolean;
  mtls_enabled?: boolean;
};

export type RaftNodeSnapshot = {
  id: string;
  role: string;
  status: 'live' | 'stale' | 'candidate' | string;
  term?: number;
  last_heartbeat_age_secs?: number;
  leader_id?: string | null;
  voted_for?: string | null;
  is_leader?: boolean;
};

export type RaftStatus = {
  leader_id?: string | null;
  leader?: string | null;
  term?: number;
  commit_index?: number;
  last_applied?: number;
  log_size?: number;
  last_log_index?: number;
  active_nodes?: number;
  total_nodes?: number;
  nodes?: RaftNodeSnapshot[];
  checked_at?: number;
  error?: string;
};

export type RaftMetrics = {
  replication_count: number;
  commit_count: number;
  avg_commit_time_ms: number;
  election_count: number;
  last_commit_time_ms?: number | null;
};

export type FederationHealthResponse = {
  report: FederationHealthReport;
  raft?: RaftStatus;
};

export type FederationSyncResult = {
  peer_id: string;
  peer_url: string;
  synced: number;
  success: boolean;
  error?: string | null;
  raft_index?: number | null;
  local_merkle_before?: string;
  local_merkle_after?: string;
  remote_merkle_before?: string | null;
  remote_merkle_after?: string | null;
  merkle_match?: boolean;
  auth_used?: boolean;
  merkle_repaired?: number;
};

export type FederationSyncResponse = {
  success: boolean;
  sync_all?: boolean;
  result?: FederationSyncResult;
  results?: FederationSyncResult[];
  error?: string;
};

export type JwtClaims = {
  sub?: string;
  exp?: number;
  iat?: number;
  jti?: string | null;
  scope?: string[];
  iss?: string;
  token_type?: string;
  [k: string]: unknown;
};

export type ApiOptions = {
  baseUrl?: string;
  onUnauthorized?: () => void;
  /**
   * If true: do not attach Authorization header and use `credentials: 'include'`
   * to support httpOnly cookie auth (requires backend support).
   */
  useCookies?: boolean;
  /**
   * Network timeout for requests (ms). Prevents infinite "Validating session..." on hung connections.
   */
  timeoutMs?: number;
};

function safeJsonParse(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

function base64UrlDecodeToString(input: string): string | null {
  try {
    const s = input.replace(/-/g, '+').replace(/_/g, '/');
    const pad = s.length % 4 === 0 ? '' : '='.repeat(4 - (s.length % 4));
    const b64 = s + pad;
    // atob exists in browsers; this lib is client-side only.
    return atob(b64);
  } catch {
    return null;
  }
}

export function decodeJwtClaims(token: string): JwtClaims | null {
  const parts = token.split('.');
  if (parts.length !== 3) return null;
  const payload = base64UrlDecodeToString(parts[1]);
  if (!payload) return null;
  const v = safeJsonParse(payload);
  if (!v || typeof v !== 'object') return null;
  return v as JwtClaims;
}

export function getApiBaseUrl(): string {
  return process.env.NEXT_PUBLIC_API_URL || 'http://localhost:8080';
}

export function getWsBaseUrl(): string {
  const api = new URL(getApiBaseUrl());
  api.protocol = api.protocol === 'https:' ? 'wss:' : 'ws:';
  return api.toString().replace(/\/$/, '');
}

// ==================== Auth storage ====================
const LS_ACCESS = 'aegis_access_token';
const LS_REFRESH = 'aegis_refresh_token';

const tokenStore: { accessToken: string | null; refreshToken: string | null } = {
  accessToken: null,
  refreshToken: null,
};

function hydrateTokenFromStorage() {
  if (tokenStore.accessToken) return;
  try {
    const access =
      localStorage.getItem(LS_ACCESS) || localStorage.getItem('aegis_token');
    const refresh =
      localStorage.getItem(LS_REFRESH) || localStorage.getItem('aegis_refresh_token');
    if (access) tokenStore.accessToken = access;
    if (refresh) tokenStore.refreshToken = refresh;
  } catch {
    /* ignore */
  }
}

export function setTokens(accessToken: string, refreshToken?: string) {
  tokenStore.accessToken = accessToken || null;
  if (refreshToken !== undefined) tokenStore.refreshToken = refreshToken || null;
  try {
    if (accessToken) {
      localStorage.setItem(LS_ACCESS, accessToken);
      localStorage.setItem('aegis_token', accessToken);
    } else {
      localStorage.removeItem(LS_ACCESS);
      localStorage.removeItem('aegis_token');
    }
    if (refreshToken !== undefined) {
      if (refreshToken) {
        localStorage.setItem(LS_REFRESH, refreshToken);
        localStorage.setItem('aegis_refresh_token', refreshToken);
      } else {
        localStorage.removeItem(LS_REFRESH);
        localStorage.removeItem('aegis_refresh_token');
      }
    }
  } catch {
    /* ignore */
  }
}

export function getAccessToken(): string | null {
  hydrateTokenFromStorage();
  return tokenStore.accessToken;
}

export function getRefreshToken(): string | null {
  hydrateTokenFromStorage();
  return tokenStore.refreshToken;
}

export function clearAuthStorage() {
  tokenStore.accessToken = null;
  tokenStore.refreshToken = null;
  try {
    localStorage.removeItem(LS_ACCESS);
    localStorage.removeItem(LS_REFRESH);
    localStorage.removeItem('aegis_token');
    localStorage.removeItem('aegis_refresh_token');
    localStorage.removeItem('aegis_role');
  } catch {
    /* ignore */
  }
}

/** Открыть кабинет (логин — overlay на /dashboard/overview, не отдельная страница). */
export function redirectToLogin() {
  if (typeof window === 'undefined') return;
  const onDashboard = window.location.pathname.startsWith('/dashboard');
  if (onDashboard) {
    window.dispatchEvent(new CustomEvent('aegis_unauthorized'));
    return;
  }
  window.location.href = '/dashboard/overview';
}

export function handleSessionExpired() {
  clearAuthStorage();
  redirectToLogin();
}

export function logout() {
  clearAuthStorage();
  redirectToLogin();
}

function emitApiOverlay(status: number, message: string) {
  if (typeof window === 'undefined') return;
  window.dispatchEvent(
    new CustomEvent('aegis_api_error', {
      detail: { status, message },
    })
  );
}

export class ApiClient {
  private baseUrl: string;
  private onUnauthorized: () => void;
  private useCookies: boolean;
  private timeoutMs: number;
  private static refreshInFlight: Promise<string | null> | null = null;

  constructor(opts: ApiOptions = {}) {
    this.baseUrl = (opts.baseUrl || getApiBaseUrl()).replace(/\/$/, '');
    this.onUnauthorized = opts.onUnauthorized || handleSessionExpired;
    this.useCookies =
      opts.useCookies ?? (process.env.NEXT_PUBLIC_AUTH_MODE === 'cookie');
    this.timeoutMs = Number.isFinite(opts.timeoutMs) ? (opts.timeoutMs as number) : 8000;
  }

  private authHeader(): Record<string, string> {
    if (this.useCookies) return {};
    const token = getAccessToken() || '';
    return token ? { Authorization: `Bearer ${token}` } : {};
  }

  private async refreshAccessToken(): Promise<string | null> {
    if (this.useCookies) return null;

    if (ApiClient.refreshInFlight) return ApiClient.refreshInFlight;
    ApiClient.refreshInFlight = (async () => {
      const refresh = getRefreshToken();
      if (!refresh) return null;
      try {
        const out = await this.request<{ access_token?: string; accessToken?: string }>(
          '/api/refresh',
          {
            method: 'POST',
            // Important: bypass recursive refresh on 401 here
            headers: { 'x-aegis-refresh': '1' } as any,
            json: { refresh_token: refresh },
          }
        );
        const newAccess = String((out as any).access_token || (out as any).accessToken || '');
        if (!newAccess) return null;
        setTokens(newAccess);
        return newAccess;
      } catch {
        return null;
      } finally {
        ApiClient.refreshInFlight = null;
      }
    })();

    return ApiClient.refreshInFlight;
  }

  async request<T = any>(
    path: string,
    init: RequestInit & { json?: unknown } = {}
  ): Promise<T> {
    const url = `${this.baseUrl}${path.startsWith('/') ? '' : '/'}${path}`;

    const headers: Record<string, string> = {
      ...(init.headers as any),
      ...this.authHeader(),
    };

    let body = init.body;
    if (init.json !== undefined) {
      headers['Content-Type'] = headers['Content-Type'] || 'application/json';
      body = JSON.stringify(init.json);
    }

    const isRefreshCall = headers['x-aegis-refresh'] === '1';
    if (isRefreshCall) delete headers['x-aegis-refresh'];

    const controller = new AbortController();
    const timeout = window.setTimeout(() => controller.abort(), this.timeoutMs);
    let res: Response;
    try {
      res = await fetch(url, {
        ...init,
        headers,
        body,
        credentials: this.useCookies ? 'include' : init.credentials,
        signal: init.signal ?? controller.signal,
      });
    } catch (e: any) {
      if (e?.name === 'AbortError') {
        throw new Error(`Request timeout after ${this.timeoutMs}ms`);
      }
      throw e;
    } finally {
      window.clearTimeout(timeout);
    }

    if (res.status === 401 && !isRefreshCall) {
      // Try refresh once, then retry original request.
      const refreshed = await this.refreshAccessToken();
      if (refreshed) {
        return this.request<T>(path, init);
      }
      this.onUnauthorized();
      throw new Error('Unauthorized');
    }

    const text = await res.text();
    const data = safeJsonParse(text);

    if (!res.ok) {
      const errBody =
        data && typeof data === 'object'
          ? (data as { error?: string; message?: string }).error ||
            (data as { message?: string }).message
          : undefined;
      const msg = errBody ? String(errBody) : `HTTP ${res.status}`;
      const err = new Error(msg) as Error & { httpStatus?: number };
      err.httpStatus = res.status;

      if (res.status === 403 || res.status >= 500) {
        emitApiOverlay(res.status, msg);
      }
      throw err;
    }

    return data as T;
  }

  // Convenience endpoints
  getStatus() {
    return this.request<StatusResponse>('/api/status', { method: 'GET' });
  }

  getKnowledge() {
    return this.request<KnowledgeResponse>('/api/knowledge', { method: 'GET' });
  }

  getFusedThreats() {
    return this.request<FusedThreatRow[]>('/api/fused-threats', { method: 'GET' });
  }

  getReactStatus() {
    return this.request<ReactLlmStatus>('/api/react/status', { method: 'GET' });
  }

  launchReactMission(mission: string) {
    return this.request('/api/react', { method: 'POST', json: { mission } });
  }

  runScout() {
    return this.request<ScoutResult>('/api/scout', { method: 'POST', json: {} });
  }

  getBduRecent() {
    return this.request<BduRecentResponse>('/api/bdu/recent', { method: 'GET' });
  }

  getAgents() {
    return this.request('/api/agents', { method: 'GET' });
  }

  toggleAgent(id: string, action: 'start' | 'stop') {
    return this.request(`/api/agents/${id}/${action}`, { method: 'POST', json: {} });
  }

  getAuditReport() {
    return this.request('/api/audit-tail', { method: 'GET' });
  }

  getFederationNodes() {
    return this.request<FederationNode[]>('/api/federation/nodes', { method: 'GET' });
  }

  getFederationHealth() {
    return this.request<FederationHealthResponse>('/api/federation/health', { method: 'GET' });
  }

  getFederationMetrics() {
    return this.request<FederationOpsMetrics>('/api/federation/metrics', { method: 'GET' });
  }

  getRaftStatus() {
    return this.request<RaftStatus>('/api/raft/status', { method: 'GET' });
  }

  getRaftMetrics() {
    return this.request<RaftMetrics>('/api/raft/metrics', { method: 'GET' });
  }

  toggleAirGap(enabled: boolean) {
    return this.request('/api/air-gap', { method: 'POST', json: { enabled } });
  }

  syncFederationNode(opts: { peerUrl?: string; peerId?: string; syncAll?: boolean }) {
    return this.request<FederationSyncResponse>('/api/federation/sync', {
      method: 'POST',
      json: {
        peer_url: opts.peerUrl,
        peer_id: opts.peerId,
        sync_all: opts.syncAll ?? false,
      },
    });
  }

  containCluster(clusterId: string) {
    return this.request<ContainResult>('/api/contain', { method: 'POST', json: { cluster_id: clusterId } });
  }
}

export function deriveRoleFromClaims(claims: JwtClaims | null): 'operator' | 'admin' {
  const scope = Array.isArray(claims?.scope) ? claims!.scope! : [];
  // Backend scopes: tier + "read" + "threats". Treat enterprise as admin for demo gating.
  if (scope.includes('enterprise')) return 'admin';
  return 'operator';
}

