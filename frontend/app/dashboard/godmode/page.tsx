'use client';

import React, { useState } from 'react';
import { Terminal, ShieldCheck, AlertOctagon, Download, WifiOff } from 'lucide-react';
import { ApiClient } from '../../../lib/api';

const api = new ApiClient();

export default function GodMode() {
  const [isAirGapped, setIsAirGapped] = useState(false);

  const toggleAirGap = async () => {
    try {
      const newState = !isAirGapped;
      await api.toggleAirGap(newState);
      setIsAirGapped(newState);
      alert(newState ? 'ВНИМАНИЕ: Система переведена в изолированный режим (Air-Gapped).' : 'Система выведена из изолированного режима.');
      // Force reload to update layout header
      window.location.reload();
    } catch (e) {
      alert('Ошибка при переключении режима');
    }
  };

  const downloadReport = async () => {
    try {
      const data = await api.getAuditReport();
      const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `AEGIS_FSTEK_AUDIT_${new Date().toISOString().slice(0,10)}.json`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (e) {
      alert('Ошибка при выгрузке отчета');
    }
  };

  return (
    <div className="max-w-[1200px] mx-auto">
      <div className="mb-12 flex items-end justify-between">
        <div>
          <div className="font-mono text-xs tracking-[4px] text-[#ffb4ab] mb-2">GOD MODE • MAXIMUM AUDIT</div>
          <h1 className="text-4xl font-bold tracking-tight">God Mode Terminal</h1>
        </div>
        <div className="flex gap-4">
          <button 
            onClick={downloadReport}
            className="flex items-center gap-2 px-5 py-2 rounded-xl border border-white/20 hover:bg-white/5 text-xs font-mono tracking-widest transition-colors active:scale-[0.985]"
          >
            <Download className="w-4 h-4" />
            ВЫГРУЗИТЬ ЖУРНАЛ (ГОСТ Р)
          </button>
          <div className="px-5 py-2 rounded-full border border-[#ffb4ab]/40 text-[#ffb4ab] text-xs font-mono tracking-widest flex items-center">
            RESTRICTED ACCESS
          </div>
        </div>
      </div>

      <div className="grid md:grid-cols-2 gap-6">
        <div className="glass-card rounded-3xl p-9 border border-[#ffb4ab]/20">
          <div className="flex items-center gap-3 mb-6">
            <ShieldCheck className="w-6 h-6 text-[#00F5A3]" />
            <div className="font-mono text-sm tracking-widest">SUPPLY CHAIN VERIFICATION</div>
          </div>
          <div className="text-white/50 text-sm leading-relaxed">
            cargo-deny + cargo-audit + formal verification pipeline. 
            All dependencies are continuously scanned for known CVEs and license violations.
          </div>
          <button className="mt-8 text-xs font-mono tracking-[3px] px-6 py-3 border border-white/20 rounded-2xl hover:bg-white/5 transition-colors active:scale-[0.985]">RUN FULL AUDIT</button>
        </div>

        <div className="glass-card rounded-3xl p-9 border border-[#ffb4ab]/20">
          <div className="flex items-center justify-between mb-6">
            <div className="flex items-center gap-3">
              <WifiOff className={`w-6 h-6 ${isAirGapped ? 'text-[#00F5A3]' : 'text-white/40'}`} />
              <div className="font-mono text-sm tracking-widest">AIR-GAPPED MODE</div>
            </div>
            <button 
              onClick={toggleAirGap}
              className={`w-12 h-6 rounded-full transition-colors relative ${isAirGapped ? 'bg-[#00F5A3]' : 'bg-white/20'}`}
            >
              <div className={`absolute top-1 w-4 h-4 rounded-full bg-white transition-all ${isAirGapped ? 'left-7' : 'left-1'}`} />
            </button>
          </div>
          <div className={`font-bold text-4xl tracking-tighter mb-2 ${isAirGapped ? 'text-[#00F5A3]' : 'text-white/40'}`}>
            {isAirGapped ? 'ISOLATED' : 'CONNECTED'}
          </div>
          <div className="text-white/40 text-sm">
            При включении режима система аппаратно блокирует все внешние сетевые вызовы и переходит на локальные LLM-модели.
          </div>
        </div>
      </div>

      <div className="mt-6 glass-card rounded-3xl p-9 font-mono text-sm text-white/40">
        Full God Mode CLI and formal verification reports will be available in this terminal.
      </div>
    </div>
  );
}
