'use client';

import React, { useState } from 'react';
import { X, Play, Loader2 } from 'lucide-react';
import { useToast } from './Toast';
import { ApiClient } from '../lib/api';

const api = new ApiClient();

interface Props {
  isOpen: boolean;
  onClose: () => void;
  onSuccess: (mission: string) => void;
}

export default function ReactMissionModal({ isOpen, onClose, onSuccess }: Props) {
  const { showToast } = useToast();
  const [mission, setMission] = useState('');
  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState<any>(null);

  if (!isOpen) return null;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!mission.trim()) return;

    setLoading(true);
    setResult(null);

    try {
      const data = await api.launchReactMission(mission.trim());
      setResult(data);

      if (data.status === 'accepted') {
        showToast('MISSION ACCEPTED — ReAct++ agent deployed');
        onSuccess(mission.trim());
        setTimeout(() => {
          onClose();
          setMission('');
          setResult(null);
        }, 1400);
      }
    } catch (err) {
      setResult({ error: 'Failed to connect to backend' });
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="fixed inset-0 z-[200] flex items-center justify-center bg-black/80 backdrop-blur-sm">
      <div className="glass-card w-full max-w-lg rounded-3xl p-10 relative border border-white/10">
        <button onClick={onClose} className="absolute top-6 right-6 text-white/40 hover:text-white">
          <X className="w-5 h-5" />
        </button>

        <div className="flex items-center gap-3 mb-8">
          <div className="w-10 h-10 rounded-xl bg-[#ddb7ff]/10 flex items-center justify-center">
            <Play className="w-5 h-5 text-[#ddb7ff]" />
          </div>
          <div>
            <div className="font-mono text-xs tracking-[3px] text-[#ddb7ff]">REACT++ ENGINE</div>
            <div className="text-3xl font-bold tracking-tighter">New Mission</div>
          </div>
        </div>

        <form onSubmit={handleSubmit}>
          <textarea
            value={mission}
            onChange={(e) => setMission(e.target.value)}
            placeholder="Describe the mission... e.g. 'Hunt for recent supply chain attacks in npm ecosystem and generate containment plan'"
            className="w-full h-36 bg-black/40 border border-white/10 rounded-2xl p-5 text-base resize-y focus:outline-none focus:border-[#ddb7ff]/60 placeholder:text-white/30"
            disabled={loading}
          />

          <button
            type="submit"
            disabled={loading || !mission.trim()}
            className="mt-6 w-full flex items-center justify-center gap-3 py-4 bg-[#ddb7ff] text-black rounded-2xl font-bold tracking-[3px] text-sm disabled:opacity-50 hover:bg-white transition-all active:scale-[0.985]"
          >
            {loading ? (
              <>LAUNCHING <Loader2 className="w-4 h-4 animate-spin" /></>
            ) : (
              'DEPLOY REACT++ AGENT'
            )}
          </button>
        </form>

        {result && (
          <div className="mt-6 text-center text-sm font-mono tracking-widest text-[#00F5A3]">
            {result.status === 'accepted' ? 'MISSION ACCEPTED • EXECUTING...' : result.error}
          </div>
        )}
      </div>
    </div>
  );
}
