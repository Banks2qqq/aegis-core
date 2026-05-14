'use client';

import React, { useEffect, useState } from 'react';
import { motion } from 'framer-motion';
import { 
  Shield, 
  AlertTriangle, 
  Activity, 
  Users, 
  Target, 
  Zap 
} from 'lucide-react';
import { ApiClient, getWsBaseUrl } from '../../../lib/api';
import ErrorBoundary from '../../../components/ErrorBoundary';
import LoadingSpinner from '../../../components/LoadingSpinner';

const api = new ApiClient();

interface StatusData {
  oracle_alive: boolean;
  active_sentinels: number;
  threats_blocked: number;
  osint_documents: number;
  darknet_documents: number;
  shield_active: boolean;
  version: string;
}

export default function OverviewWarRoom() {
  const [status, setStatus] = useState<StatusData | null>(null);
  const [lastUpdate, setLastUpdate] = useState<Date>(new Date());
  const [liveEvents, setLiveEvents] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);

  const fetchStatus = async () => {
    try {
      setLoading(true);
      const data: StatusData = await api.getStatus();
      setStatus(data);
      setLastUpdate(new Date());
    } catch (e) {
      console.error('Failed to fetch status', e);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchStatus();
    const interval = setInterval(fetchStatus, 10000);

    // Live WebSocket events
    let ws: WebSocket | null = null;
    try {
      ws = new WebSocket(`${getWsBaseUrl()}/ws`);
      ws.onmessage = (event) => {
        try {
          const msg = JSON.parse(event.data);
          if (msg.type === 'alert' || msg.type === 'threat') {
            const text = typeof msg.data === 'string' ? msg.data : JSON.stringify(msg.data);
            setLiveEvents(prev => [text.slice(0, 140), ...prev].slice(0, 8));
          }
        } catch (_) {}
      };
    } catch (_) {}

    return () => {
      clearInterval(interval);
      ws?.close();
      setLiveEvents([]);
    };
  }, []);

  const kpis = status ? [
    { label: "ACTIVE THREATS", value: status.threats_blocked.toString(), change: "+0", icon: AlertTriangle, color: "#ffb4ab" },
    { label: "AGENTS ONLINE", value: status.active_sentinels.toString(), change: "+1", icon: Users, color: "#ddb7ff" },
    { label: "FUSION SCORE", value: status.shield_active ? "98.4" : "72.0", change: "+2.1", icon: Target, color: "#a4c9ff" },
    { label: "EVENTS / MIN", value: Math.floor(status.osint_documents / 10 + 120).toString(), change: "+18", icon: Activity, color: "#fabc4e" },
  ] : [
    { label: "ACTIVE THREATS", value: "—", change: "", icon: AlertTriangle, color: "#ffb4ab" },
    { label: "AGENTS ONLINE", value: "—", change: "", icon: Users, color: "#ddb7ff" },
    { label: "FUSION SCORE", value: "—", change: "", icon: Target, color: "#a4c9ff" },
    { label: "EVENTS / MIN", value: "—", change: "", icon: Activity, color: "#fabc4e" },
  ];

  return (
    <ErrorBoundary fallbackTitle="OVERVIEW UI ERROR">
      {loading && !status ? (
        <div className="flex h-[60vh] items-center justify-center">
          <LoadingSpinner label="Loading status..." />
        </div>
      ) : (
    <div className="max-w-[1600px] mx-auto space-y-8">
      {/* Header */}
      <div className="flex items-end justify-between">
        <div>
          <div className="font-mono text-xs tracking-[4px] text-[#a4c9ff] mb-2">WAR ROOM • LIVE</div>
          <h1 className="text-6xl font-bold tracking-tighter">Overview</h1>
          {!status && (
            <div className="mt-3 font-mono text-xs tracking-widest text-[#ffb4ab]">
              Connecting to Oracle…
            </div>
          )}
        </div>
        <div className="text-right text-sm text-white/40 font-mono">
          LAST UPDATED: {lastUpdate.toLocaleTimeString('ru-RU')}<br />
          <span className="text-[#00F5A3] text-xs">v{status?.version || '8.7.0'}</span>
        </div>
      </div>

      {/* KPI Cards */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        {kpis.map((kpi, i) => {
          const Icon = kpi.icon;
          return (
            <motion.div 
              key={i}
              whileHover={{ y: -2 }}
              className="glass-card rounded-3xl p-6 border border-white/10"
            >
              <div className="flex justify-between items-start mb-8">
                <div className="text-xs tracking-[2px] text-white/50 font-mono">{kpi.label}</div>
                <Icon className="w-5 h-5" style={{ color: kpi.color }} />
              </div>
              <div className="flex items-baseline gap-3">
                <div className="text-6xl font-bold tracking-tighter tabular-nums">{kpi.value}</div>
                {kpi.change && (
                  <div className={`text-sm font-mono ${kpi.change.startsWith('+') ? 'text-[#00F5A3]' : 'text-[#ffb4ab]'}`}>
                    {kpi.change}
                  </div>
                )}
              </div>
            </motion.div>
          );
        })}
      </div>

      {/* Main Grid */}
      <div className="grid lg:grid-cols-12 gap-4">
        
        {/* Live Threat Map */}
        <div className="lg:col-span-7 glass-card rounded-3xl p-8 relative overflow-hidden min-h-[420px]">
          <div className="hud-corner corner-tl" />
          <div className="hud-corner corner-tr" />
          <div className="hud-corner corner-bl" />
          <div className="hud-corner corner-br" />

          <div className="flex items-center justify-between mb-6">
            <div>
              <div className="font-mono text-xs tracking-[3px] text-[#a4c9ff]">GLOBAL THREAT SURFACE</div>
              <div className="text-3xl font-bold tracking-tight mt-1">Threat Map</div>
            </div>
            <div className="px-4 py-1 rounded-full bg-[#ffb4ab]/10 text-[#ffb4ab] text-xs font-mono tracking-widest border border-[#ffb4ab]/30">
              47 ACTIVE NODES
            </div>
          </div>

          <div className="absolute inset-0 flex items-center justify-center opacity-40">
            <div className="relative w-[380px] h-[380px] rounded-full border border-white/10 flex items-center justify-center">
              <div className="absolute w-3 h-3 bg-[#ddb7ff] rounded-full animate-ping" style={{ top: '30%', left: '40%' }} />
              <div className="absolute w-2 h-2 bg-[#fabc4e] rounded-full animate-pulse" style={{ top: '65%', left: '72%' }} />
              <div className="absolute w-2.5 h-2.5 bg-[#a4c9ff] rounded-full" style={{ top: '22%', left: '68%' }} />
              <Target className="w-16 h-16 text-white/10" />
            </div>
          </div>

          <div className="absolute bottom-8 right-8 text-xs font-mono text-white/30 tracking-widest">
            REAL-TIME • 12 REGIONS
          </div>
        </div>

        {/* Agent Status */}
        <div className="lg:col-span-5 glass-card rounded-3xl p-8 flex flex-col">
          <div className="flex items-center justify-between mb-8">
            <div>
              <div className="font-mono text-xs tracking-[3px] text-[#ddb7ff]">AUTONOMOUS SWARM</div>
              <div className="text-3xl font-bold tracking-tight mt-1">Agent Status</div>
            </div>
            <div className="text-right">
              <div className="text-4xl font-bold text-[#ddb7ff]">42</div>
              <div className="text-xs text-white/40">/ 50</div>
            </div>
          </div>

          <div className="space-y-4 flex-1">
            {[
              { name: "ORACLE-7", status: "HUNTING", load: 87, color: "#ddb7ff" },
              { name: "THREAT-HUNTER-3", status: "FUSING", load: 64, color: "#a4c9ff" },
              { name: "INQUISITOR-1", status: "ANALYZING", load: 92, color: "#fabc4e" },
            ].map((agent, idx) => (
              <div key={idx} className="flex items-center gap-4 bg-white/5 rounded-2xl px-5 py-4">
                <div className="flex-1">
                  <div className="font-mono text-sm tracking-widest">{agent.name}</div>
                  <div className="text-xs text-white/40 mt-0.5">{agent.status}</div>
                </div>
                <div className="w-24 h-1.5 bg-white/10 rounded-full overflow-hidden">
                  <div 
                    className="h-full transition-all" 
                    style={{ width: `${agent.load}%`, backgroundColor: agent.color }}
                  />
                </div>
                <div className="font-mono text-xs w-9 text-right tabular-nums">{agent.load}%</div>
              </div>
            ))}
          </div>

          <button className="mt-6 w-full py-3 border border-white/20 rounded-2xl text-xs tracking-[2px] hover:bg-white/5 transition-all font-mono">
            VIEW ALL AGENTS →
          </button>
        </div>

        {/* Recent Events */}
        <div className="lg:col-span-12 glass-card rounded-3xl p-8">
          <div className="flex items-center justify-between mb-6">
            <div>
              <div className="font-mono text-xs tracking-[3px] text-[#fabc4e]">LIVE FEED</div>
              <div className="text-3xl font-bold tracking-tight">Recent Events</div>
            </div>
            <button className="text-xs font-mono tracking-widest px-6 py-2 border border-white/20 rounded-full hover:bg-white/5">
              VIEW FULL LOG
            </button>
          </div>

          <div className="font-mono text-sm space-y-3 text-white/70">
            {liveEvents.map((line, i) => (
              <div key={i} className="flex gap-4 border-l-2 border-white/10 pl-4 py-1">
                {line}
              </div>
            ))}
            {liveEvents.length === 0 && <div className="text-white/40">Waiting for live events...</div>}
          </div>
        </div>
      </div>
    </div>
      )}
    </ErrorBoundary>
  );
}
