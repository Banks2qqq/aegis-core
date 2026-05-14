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

// ==================== Zero-Trust storage ====================
// P0 demo-ready: keep tokens in-memory (minimize persistence surface).
// Note: page reload clears tokens → user re-authenticates (acceptable for demos).
const tokenStore: { accessToken: string | null; refreshToken: string | null } = {
  accessToken: null,
  refreshToken: null,
};

export function setTokens(accessToken: string, refreshToken?: string) {
  tokenStore.accessToken = accessToken || null;
  if (refreshToken !== undefined) tokenStore.refreshToken = refreshToken || null;
}

export function getAccessToken(): string | null {
  return tokenStore.accessToken;
}

export function getRefreshToken(): string | null {
  return tokenStore.refreshToken;
}

export function clearAuthStorage() {
  tokenStore.accessToken = null;
  tokenStore.refreshToken = null;
  // keep best-effort cleanup for legacy sessions
  try {
    localStorage.removeItem('aegis_token');
    localStorage.removeItem('aegis_refresh_token');
    localStorage.removeItem('aegis_role');
  } catch {}
}

export function redirectToLogin() {
  if (typeof window === 'undefined') return;
  window.location.href = '/dashboard/login';
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
    this.onUnauthorized = opts.onUnauthorized || logout;
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
      const msg =
        (data && typeof data === 'object' && (data as any).message && String((data as any).message)) ||
        `HTTP ${res.status}`;

      if (res.status === 403 || res.status >= 500) {
        emitApiOverlay(res.status, msg);
      }
      throw new Error(msg);
    }

    return data as T;
  }

  // Convenience endpoints
  getStatus() {
    return this.request('/api/status', { method: 'GET' });
  }

  getFusedThreats() {
    return this.request('/api/fused-threats', { method: 'GET' });
  }

  launchReactMission(mission: string) {
    return this.request('/api/react', { method: 'POST', json: { mission } });
  }
}

export function deriveRoleFromClaims(claims: JwtClaims | null): 'operator' | 'admin' {
  const scope = Array.isArray(claims?.scope) ? claims!.scope! : [];
  // Backend scopes: tier + "read" + "threats". Treat enterprise as admin for demo gating.
  if (scope.includes('enterprise')) return 'admin';
  return 'operator';
}

