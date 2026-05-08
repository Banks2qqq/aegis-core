'use client';
import { useState, useEffect, useCallback } from 'react';

export function useAegisLink() {
  const [status, setStatus] = useState({
    oracle_alive: false,
    active_sentinels: 0,
    threats_blocked: 0,
    osint_documents: 0,
    darknet_documents: 0,
  });
  const [alerts, setAlerts] = useState([]);
  const [connected, setConnected] = useState(false);
  const [bloomTrigger, setBloomTrigger] = useState(0); // Для вспышки

  // Начальный статус через HTTP
  useEffect(() => {
    fetch('http://localhost:8080/api/status')
      .then(r => r.json())
      .then(d => setStatus(prev => ({ ...prev, ...d })))
      .catch(() => {});
  }, []);

  // WebSocket для алертов
  useEffect(() => {
    let ws;
    let reconnect;

    const connect = () => {
      ws = new WebSocket('ws://localhost:8080/ws');

      ws.onopen = () => setConnected(true);

      ws.onmessage = (e) => {
        const msg = JSON.parse(e.data);
        if (msg.type === 'init') {
          setStatus(prev => ({ ...prev, ...msg.data }));
        } else if (msg.type === 'alert') {
          // Новый алерт
          setAlerts(prev => [msg.data, ...prev].slice(0, 50));
          setStatus(prev => ({ ...prev, threats_blocked: prev.threats_blocked + 1 }));
          // Триггер вспышки (меняется число → Scene реагирует)
          setBloomTrigger(t => t + 1);
        }
      };

      ws.onclose = () => {
        setConnected(false);
        reconnect = setTimeout(connect, 3000);
      };
      ws.onerror = () => ws.close();
    };

    connect();
    return () => {
      clearTimeout(reconnect);
      if (ws) ws.close();
    };
  }, []);

  return { status, alerts, connected, bloomTrigger };
}