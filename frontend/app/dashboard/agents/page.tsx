'use client';

import React, { useEffect, useState } from 'react';
import { Brain, Play, Pause, Zap, AlertTriangle } from 'lucide-react';
import { ApiClient } from '../../../lib/api';
import ErrorBoundary from '../../../components/ErrorBoundary';
import LoadingSpinner from '../../../components/LoadingSpinner';

const api = new ApiClient();

type AgentStatus = {
  id: string;
  role: string;
  status: string;
  load: number;
  critic: number;
};

export default function ReActAgents() {
  const [agents, setAgents] = useState<AgentStatus[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string>('');

  useEffect(() => {
    let cancelled = false;

    async function fetchAgents() {
      try {
        setError('');
        const data = await api.request<AgentStatus[]>('/api/agents', { method: 'GET' });
        if (!cancelled) setAgents(data);
      } catch (e) {
        if (!cancelled) setError('Endpoint not implemented yet: GET /api/agents');
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    fetchAgents();
    const t = setInterval(fetchAgents, 15000);
    return () => {
      cancelled = true;
      clearInterval(t);
    };
  }, []);

  return (
    <ErrorBoundary fallbackTitle="AGENTS UI ERROR">
    <div className="max-w-[1400px] mx-auto">
      <div className="mb-10">
        <div className="font-mono text-xs tracking-[4px] text-[#ddb7ff] mb-3">AUTONOMOUS REASONING ENGINE</div>
        <h1 className="text-6xl font-bold tracking-tighter">ReAct++ Agents</h1>
        <p className="text-xl text-white/40 mt-3">Thought → Critic → Action • Kill Switch enabled • MCTS optimized</p>
      </div>

      <div className="grid md:grid-cols-2 gap-4 mb-8">
        {loading && agents.length === 0 && (
          <div className="md:col-span-2 glass-card rounded-3xl p-10 text-white/40 font-mono">
            <LoadingSpinner label="Loading agent telemetry..." />
          </div>
        )}
        {!loading && error && (
          <div className="md:col-span-2 glass-card rounded-3xl p-10 border border-[#ffb4ab]/30">
            <div className="flex items-center gap-3 text-[#ffb4ab] font-mono tracking-widest text-xs">
              <AlertTriangle className="w-4 h-4" />
              {error}
            </div>
            <div className="text-white/40 text-sm mt-3">
              Implement this endpoint in backend (`backend/src/agent/server.rs`) to make the Agents page fully live.
            </div>
          </div>
        )}
        {agents.map((agent, i) => (
          <div key={i} className="glass-card rounded-3xl p-8 border-l-4 border-[#ddb7ff]/60">
            <div className="flex justify-between items-start">
              <div>
                <div className="font-mono text-xl tracking-[2px]">{agent.id}</div>
                <div className="text-white/50 text-sm mt-1">{agent.role}</div>
              </div>
              <div className="text-right">
                <div className="text-4xl font-bold tabular-nums tracking-tighter text-[#ddb7ff]">{agent.load}</div>
                <div className="text-[10px] text-white/40 -mt-1">LOAD</div>
              </div>
            </div>

            <div className="mt-8 flex items-center justify-between text-xs font-mono">
              <div className="px-4 py-1 rounded-full bg-white/5 border border-white/10">{agent.status}</div>
              <div>CRITIC SCORE: <span className="text-[#00F5A3]">{(agent.critic * 100).toFixed(0)}%</span></div>
            </div>

            <div className="mt-6 h-px bg-white/10" />
            
            <div className="mt-6 flex gap-3">
              <button className="flex-1 py-3 text-xs tracking-widest border border-white/20 rounded-2xl hover:bg-white/5">VIEW LOGS</button>
              <button className="flex-1 py-3 text-xs tracking-widest bg-white/10 rounded-2xl hover:bg-white/20">INTERVENE</button>
            </div>
          </div>
        ))}
      </div>

      <div className="flex gap-4">
        <button className="flex items-center gap-3 px-10 py-4 bg-[#ddb7ff] text-black rounded-2xl font-bold tracking-[3px] text-sm">
          <Play className="w-4 h-4" /> DEPLOY NEW AGENT
        </button>
        <button className="flex items-center gap-3 px-10 py-4 border border-white/20 rounded-2xl font-mono text-sm tracking-widest hover:bg-white/5">
          <Pause className="w-4 h-4" /> PAUSE ALL
        </button>
      </div>
    </div>
    </ErrorBoundary>
  );
}
