'use client';

import { useEffect } from 'react';
import { AlertTriangle } from 'lucide-react';

export default function Error({ error, reset }: { error: Error & { digest?: string }; reset: () => void }) {
  useEffect(() => {
    console.error('[AEGIS] dashboard error:', error);
  }, [error]);

  return (
    <div className="max-w-[1200px] mx-auto">
      <div className="glass-card rounded-3xl p-12 border border-[#ffb4ab]/30">
        <div className="flex items-center gap-3 text-[#ffb4ab] font-mono tracking-widest text-xs">
          <AlertTriangle className="w-4 h-4" />
          DASHBOARD ERROR
        </div>
        <div className="text-white/50 mt-4 font-mono text-xs whitespace-pre-wrap">
          {error.message}
        </div>
        <button
          type="button"
          onClick={reset}
          className="mt-6 px-6 py-2 text-xs font-mono tracking-widest border border-white/20 rounded-xl hover:bg-white/5"
        >
          RETRY
        </button>
      </div>
    </div>
  );
}

