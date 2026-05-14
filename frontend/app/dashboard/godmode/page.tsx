'use client';

import React from 'react';
import { Terminal, ShieldCheck, AlertOctagon } from 'lucide-react';

export default function GodMode() {
  return (
    <div className="max-w-[1200px] mx-auto">
      <div className="mb-12 flex items-end justify-between">
        <div>
          <div className="font-mono text-xs tracking-[4px] text-[#ffb4ab] mb-3">GOD MODE • MAXIMUM AUDIT</div>
          <h1 className="text-6xl font-bold tracking-tighter">God Mode Terminal</h1>
        </div>
        <div className="px-5 py-2 rounded-full border border-[#ffb4ab]/40 text-[#ffb4ab] text-xs font-mono tracking-widest">RESTRICTED ACCESS</div>
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
          <button className="mt-8 text-xs font-mono tracking-[3px] px-6 py-3 border border-white/20 rounded-2xl hover:bg-white/5">RUN FULL AUDIT</button>
        </div>

        <div className="glass-card rounded-3xl p-9 border border-[#ffb4ab]/20">
          <div className="flex items-center gap-3 mb-6">
            <AlertOctagon className="w-6 h-6 text-[#ffb4ab]" />
            <div className="font-mono text-sm tracking-widest">KILL SWITCH STATUS</div>
          </div>
          <div className="font-bold text-6xl tracking-tighter text-[#ffb4ab] mb-1">ARMED</div>
          <div className="text-white/40">Any action with security_risk &gt; 0.75 is automatically blocked.</div>
        </div>
      </div>

      <div className="mt-6 glass-card rounded-3xl p-9 font-mono text-sm text-white/40">
        Full God Mode CLI and formal verification reports will be available in this terminal.
      </div>
    </div>
  );
}
