'use client';

import React, { createContext, useCallback, useContext, useEffect, useRef, useState } from 'react';

type WsStatus = 'connecting' | 'connected' | 'disconnected';

type WsContextValue = {
  status: WsStatus;
  subscribe: (handler: (msg: unknown) => void) => () => void;
};

const WsContext = createContext<WsContextValue | null>(null);

export function AegisWsProvider({ children }: { children: React.ReactNode }) {
  const [status, setStatus] = useState<WsStatus>('connecting');
  const handlersRef = useRef(new Set<(msg: unknown) => void>());

  const subscribe = useCallback((handler: (msg: unknown) => void) => {
    handlersRef.current.add(handler);
    return () => handlersRef.current.delete(handler);
  }, []);

  useEffect(() => {
    let alive = true;
    let ws: WebSocket | null = null;
    let retryTimer: ReturnType<typeof setTimeout> | null = null;
    let opened = false;

    const connect = () => {
      if (!alive) return;
      const proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
      const url = `${proto}//${window.location.host}/ws`;
      try {
        ws = new WebSocket(url);
      } catch {
        setStatus('disconnected');
        retryTimer = setTimeout(connect, 4000);
        return;
      }

      ws.onopen = () => {
        opened = true;
        if (alive) setStatus('connected');
      };
      ws.onerror = () => {
        if (alive) setStatus('disconnected');
      };
      ws.onclose = () => {
        opened = false;
        if (!alive) return;
        setStatus('disconnected');
        retryTimer = setTimeout(connect, 4000);
      };
      ws.onmessage = (event) => {
        try {
          const msg = JSON.parse(event.data);
          handlersRef.current.forEach((h) => h(msg));
        } catch {
          /* ignore */
        }
      };
    };

    connect();

    return () => {
      alive = false;
      if (retryTimer) clearTimeout(retryTimer);
      if (ws) {
        const sock = ws;
        ws = null;
        sock.onopen = null;
        sock.onclose = null;
        sock.onerror = null;
        sock.onmessage = null;
        if (opened) sock.close(1000, 'dashboard unmount');
      }
    };
  }, []);

  return <WsContext.Provider value={{ status, subscribe }}>{children}</WsContext.Provider>;
}

export function useAegisWs() {
  const ctx = useContext(WsContext);
  if (!ctx) throw new Error('useAegisWs requires AegisWsProvider');
  return ctx;
}

export function useAegisWebSocket(onMessage: (data: unknown) => void, enabled = true) {
  const { status, subscribe } = useAegisWs();
  const onMessageRef = useRef(onMessage);
  onMessageRef.current = onMessage;

  useEffect(() => {
    if (!enabled) return;
    return subscribe((msg) => onMessageRef.current(msg));
  }, [enabled, subscribe]);

  return status;
}
