'use client';
import { useState, useEffect } from 'react';

const TIERS = {
  STARTER: { name: 'Starter', price: 0, agents: 1, nodes: 500, color: '#00ffff' },
  PRO: { name: 'Professional', price: 199, agents: 5, nodes: 2500, color: '#ff00ff' },
  ENTERPRISE: { name: 'Enterprise', price: 'Custom', agents: 'Unlimited', nodes: 5000, color: '#ffffff' }
};

export default function PriceConfigurator({ onConfigChange }) {
  const [tier, setTier] = useState('PRO');
  useEffect(() => { onConfigChange(TIERS[tier]); }, [tier]);

  return (
    <div className="flex flex-col gap-6 p-8 bg-black/40 backdrop-blur-xl border border-cyan-500/30 rounded-2xl text-white max-w-2xl">
      <h2 className="text-2xl font-bold">CONFIGURATOR</h2>
      <div className="grid grid-cols-3 gap-4">
        {Object.keys(TIERS).map(k => (
          <div key={k} onClick={() => setTier(k)}
            className={`cursor-pointer p-4 border-2 rounded-xl text-center ${tier === k ? 'border-cyan-400 bg-cyan-400/10' : 'border-gray-800'}`}>
            <div className="text-xs text-cyan-400 uppercase mb-2">{TIERS[k].name}</div>
            <div className="text-3xl font-black">${TIERS[k].price}</div>
            <div className="mt-4 text-sm text-gray-400">Agents: {TIERS[k].agents}</div>
            <div className="text-sm text-gray-400">Nodes: {TIERS[k].nodes}</div>
          </div>
        ))}
      </div>
      <button className="w-full py-4 bg-cyan-500 hover:bg-cyan-400 text-black font-black uppercase rounded-lg">Activate AEGIS</button>
    </div>
  );
}