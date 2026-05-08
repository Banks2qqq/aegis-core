'use client';
import { useState, useEffect } from 'react';
import dynamic from 'next/dynamic';
import { useAegisLink } from '@/components/useAegisLink';

const AegisScene = dynamic(() => import('@/components/AegisScene'), { ssr: false });
const PriceConfigurator = dynamic(() => import('@/components/PriceConfigurator'), { ssr: false });

export default function Home() {
  const { status, alerts, bloomTrigger } = useAegisLink();
  const [config, setConfig] = useState({ nodes: 1000, agents: 1, color: '#00ffff' });
  const [progress, setProgress] = useState(0);

  useEffect(() => {
    const cb = () => {
      const max = document.body.scrollHeight - window.innerHeight;
      setProgress(Math.min(window.scrollY / (max || 2000), 1));
    };
    window.addEventListener('scroll', cb, { passive: true });
    return () => window.removeEventListener('scroll', cb);
  }, []);

  return (
    <main className="relative text-white">
      <AegisScene config={config} scrollProgress={progress} bloomTrigger={bloomTrigger} />

      <div className="min-h-screen flex flex-col items-center justify-center">
        <h1 className="text-7xl font-black bg-gradient-to-r from-cyan-400 to-emerald-400 bg-clip-text text-transparent">AEGIS</h1>
        <p className="text-xl text-gray-400 mt-4 tracking-widest">IMMUNE SYSTEM</p>
        <div className="mt-8 font-mono text-sm text-gray-500">
          {status.oracle_alive ? 'ORACLE ONLINE' : 'ORACLE OFFLINE'} | Blocked: {status.threats_blocked}
        </div>
      </div>

      <div className="min-h-screen flex items-center justify-center">
        <PriceConfigurator onConfigChange={setConfig} />
      </div>

      {/* Лог алертов */}
      <div className="min-h-screen flex items-center justify-center">
        <div className="w-full max-w-4xl space-y-8">
          <div className="grid grid-cols-3 gap-8">
            <div className="p-8 bg-black/40 backdrop-blur border border-cyan-500/30 rounded-2xl text-center">
              <div className="text-5xl font-black text-cyan-400">{status.osint_documents}</div>
              <div className="text-sm text-gray-400 mt-2">OSINT</div>
            </div>
            <div className="p-8 bg-black/40 backdrop-blur border border-pink-500/30 rounded-2xl text-center">
              <div className="text-5xl font-black text-pink-400">{status.darknet_documents}</div>
              <div className="text-sm text-gray-400 mt-2">DARKNET</div>
            </div>
            <div className="p-8 bg-black/40 backdrop-blur border border-emerald-500/30 rounded-2xl text-center">
              <div className="text-5xl font-black text-emerald-400">{status.threats_blocked}</div>
              <div className="text-sm text-gray-400 mt-2">BLOCKED</div>
            </div>
          </div>

          {/* Терминал с алертами */}
          <div className="bg-black/60 backdrop-blur border border-gray-700 rounded-2xl p-6 font-mono text-sm">
            <div className="text-gray-500 mb-4">AEGIS_COMMAND_CENTER — alerts</div>
            {alerts.length === 0 && <div className="text-gray-600">No alerts yet. System is clear.</div>}
            {alerts.slice(0, 10).map((a, i) => (
              <div key={i} className="text-green-400/80 mb-1">
                [{new Date().toLocaleTimeString()}] {typeof a === 'string' ? a : JSON.stringify(a)}
              </div>
            ))}
          </div>
        </div>
      </div>
    </main>
  );
}