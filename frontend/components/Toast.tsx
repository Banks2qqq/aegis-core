'use client';

import React, { createContext, useContext, useState, useCallback } from 'react';
import { CheckCircle, X } from 'lucide-react';

type Toast = { id: number; message: string; type?: 'success' | 'error' };

const ToastContext = createContext<{
  showToast: (message: string, type?: 'success' | 'error') => void;
}>({ showToast: () => {} });

export const useToast = () => useContext(ToastContext);

export function ToastProvider({ children }: { children: React.ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);

  const showToast = useCallback((message: string, type: 'success' | 'error' = 'success') => {
    const id = Date.now();
    setToasts(t => [...t, { id, message, type }]);
    setTimeout(() => setToasts(t => t.filter(x => x.id !== id)), 4500);
  }, []);

  return (
    <ToastContext.Provider value={{ showToast }}>
      {children}
      <div className="fixed bottom-6 right-6 z-[300] space-y-3">
        {toasts.map(t => (
          <div key={t.id} className="flex items-center gap-3 px-6 py-4 rounded-2xl bg-[#16111b] border border-white/10 shadow-2xl text-sm font-mono tracking-widest">
            <CheckCircle className="w-4 h-4 text-[#00F5A3]" />
            <span>{t.message}</span>
            <button onClick={() => setToasts(ts => ts.filter(x => x.id !== t.id))} className="ml-4 text-white/40 hover:text-white">
              <X className="w-3.5 h-3.5" />
            </button>
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  );
}
