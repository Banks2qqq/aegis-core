'use client';

import React, { useEffect, useState } from 'react';
import { Brain, Target, Zap, Play, Pause } from 'lucide-react';
import { ApiClient } from '../../../lib/api';
import { dispatchOpenReactModal } from '../../../lib/aegisEvents';

const api = new ApiClient();

export default function AgentsPage() {
  const [agents, setAgents] = useState<any[]>([]);
  const [loading, setLoading] = useState(true);

  const loadAgents = async () => {
    try {
      const data = await api.getAgents();
      setAgents(data || []);
    } catch (error) {
      console.error('Failed to load agents:', error);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    let mounted = true;
    setTimeout(() => {
      if (mounted) {
        loadAgents();
      }
    }, 0);
    return () => { mounted = false; };
  }, []);

  const toggleAgent = async (id: string, action: 'start' | 'stop') => {
    try {
      const res: any = await api.toggleAgent(id, action);
      await loadAgents();
      const label = res?.status === 'active' ? 'запущен' : 'остановлен';
      alert(`Агент ${id} ${label}`);
    } catch (error) {
      alert(`Не удалось ${action === 'start' ? 'запустить' : 'остановить'} агента`);
    }
  };

  if (loading) {
    return <div className="flex justify-center py-20">Загрузка...</div>;
  }

  return (
    <div className="space-y-8">
      <div>
        <div className="font-mono text-xs tracking-[4px] text-[#00F5A3] mb-2">AI AGENTS</div>
        <h1 className="text-4xl font-bold tracking-tight">ReAct++ Agents</h1>
        <p className="text-white/60 mt-2">ReAct++ • Critic • MCTS • Autonomous Agents</p>
      </div>

      <div className="flex justify-end">
        <button
          type="button"
          onClick={() => dispatchOpenReactModal()}
          className="inline-flex items-center gap-2 px-5 py-2.5 rounded-xl border border-[#ddb7ff]/40 text-[#ddb7ff] font-mono text-xs tracking-widest hover:bg-[#ddb7ff]/10 transition-all"
        >
          <Play className="w-4 h-4" />
          НОВАЯ REACT++ МИССИЯ
        </button>
      </div>

      <div className="grid gap-6">
        {agents.length > 0 ? (
          agents.map((agent) => (
            <div key={agent.id} className="glass-card rounded-3xl p-8">
              <div className="flex items-start justify-between mb-6">
                <div>
                  <div className="flex items-center gap-3">
                    <div className="text-[#00F5A3]">
                      <Brain className="w-6 h-6" />
                    </div>
                    <h3 className="text-2xl font-semibold">{agent.name}</h3>
                    <div className={`px-3 py-1 rounded-full text-xs ${
                      agent.status === 'active' || agent.status === 'running'
                        ? 'bg-[#00F5A3]/20 text-[#00F5A3]'
                        : agent.status === 'error'
                          ? 'bg-red-500/20 text-red-300'
                          : 'bg-white/10 text-white/70'
                    }`}>
                      {agent.status}
                    </div>
                  </div>
                  <div className="text-white/60 mt-1 font-mono text-sm">{agent.id}</div>
                </div>

                <button
                  onClick={() => toggleAgent(agent.id, agent.status === 'active' || agent.status === 'running' ? 'stop' : 'start')}
                  className="flex items-center gap-2 px-4 py-2 bg-white/10 hover:bg-white/20 rounded-xl transition-colors active:scale-[0.985]"
                >
                  {agent.status === 'active' || agent.status === 'running' ? (
                    <><Pause className="w-4 h-4" /> Остановить</>
                  ) : (
                    <><Play className="w-4 h-4" /> Запустить</>
                  )}
                </button>
              </div>

              {/* ReAct++ статус */}
              <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
                <div className="bg-black/40 rounded-2xl p-6">
                  <div className="flex items-center gap-3 mb-4">
                    <Target className="w-5 h-5 text-[#00F5A3]" />
                    <div className="font-semibold">ReAct++ Engine</div>
                  </div>
                  <div className="text-sm text-white/70 space-y-1">
                    <div>Текущая задача: <span className="text-white">{agent.current_task || "—"}</span></div>
                    <div>Итераций: <span className="text-white">{agent.react_iterations || 0}</span></div>
                  </div>
                </div>

                <div className="bg-black/40 rounded-2xl p-6">
                  <div className="flex items-center gap-3 mb-4">
                    <Brain className="w-5 h-5 text-[#8B5CF6]" />
                    <div className="font-semibold">Critic Agent</div>
                  </div>
                  <div className="text-sm text-white/70 space-y-1">
                    <div>Последняя оценка: <span className="text-white">{agent.last_critic_score || "—"}</span></div>
                    <div>Решений: <span className="text-white">{agent.critic_decisions || 0}</span></div>
                  </div>
                </div>

                <div className="bg-black/40 rounded-2xl p-6">
                  <div className="flex items-center gap-3 mb-4">
                    <Zap className="w-5 h-5 text-[#F59E0B]" />
                    <div className="font-semibold">MCTS</div>
                  </div>
                  <div className="text-sm text-white/70 space-y-1">
                    <div>Дерево поиска: <span className="text-white">{agent.mcts_nodes || 0}</span> узлов</div>
                    <div>Лучший путь: <span className="text-white">{agent.best_path_score || "—"}</span></div>
                  </div>
                </div>
              </div>
            </div>
          ))
        ) : (
          <div className="text-center py-20 text-white/50">
            Нет активных агентов
          </div>
        )}
      </div>
    </div>
  );
}
