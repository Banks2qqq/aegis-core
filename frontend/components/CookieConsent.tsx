'use client';

import React, { useEffect, useMemo, useState } from 'react';
import Link from 'next/link';
import { motion, AnimatePresence } from 'framer-motion';

type Consent = {
  necessary: true;
  analytics: boolean;
  marketing: boolean;
  updatedAt: number; // ms epoch
};

const STORAGE_KEY = 'cookie_consent';

function readConsent(): Consent | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return null;
    const v = JSON.parse(raw) as Partial<Consent> | null;
    if (!v || typeof v !== 'object') return null;
    if (v.necessary !== true) return null;
    if (typeof v.analytics !== 'boolean') return null;
    if (typeof v.marketing !== 'boolean') return null;
    if (typeof v.updatedAt !== 'number') return null;
    return v as Consent;
  } catch {
    return null;
  }
}

function writeConsent(consent: Omit<Consent, 'updatedAt'>) {
  const c: Consent = { ...consent, updatedAt: Date.now() };
  localStorage.setItem(STORAGE_KEY, JSON.stringify(c));
  window.dispatchEvent(new CustomEvent('aegis_consent_updated', { detail: c }));
}

export default function CookieConsent() {
  const [visible, setVisible] = useState(false);
  const [openSettings, setOpenSettings] = useState(false);
  const existing = useMemo(() => (typeof window === 'undefined' ? null : readConsent()), []);
  const [analytics, setAnalytics] = useState<boolean>(existing?.analytics ?? false);
  const [marketing, setMarketing] = useState<boolean>(existing?.marketing ?? false);

  useEffect(() => {
    if (typeof window === 'undefined') return;
    const c = readConsent();
    if (!c) setVisible(true);
  }, []);

  const acceptAll = () => {
    writeConsent({ necessary: true, analytics: true, marketing: true });
    setVisible(false);
    setOpenSettings(false);
  };

  const rejectNonEssential = () => {
    writeConsent({ necessary: true, analytics: false, marketing: false });
    setVisible(false);
    setOpenSettings(false);
  };

  const saveSettings = () => {
    writeConsent({ necessary: true, analytics, marketing });
    setVisible(false);
    setOpenSettings(false);
  };

  const closeMinimal = () => {
    // Treat closing as minimal consent: necessary only.
    writeConsent({ necessary: true, analytics: false, marketing: false });
    setVisible(false);
    setOpenSettings(false);
  };

  if (!visible) return null;

  return (
    <AnimatePresence>
      <motion.div
        key="cookie-banner"
        initial={{ opacity: 0, y: 18 }}
        animate={{ opacity: 1, y: 0 }}
        exit={{ opacity: 0, y: 18 }}
        transition={{ duration: 0.25 }}
        className="fixed bottom-6 left-6 right-6 z-[400] max-w-3xl mx-auto"
      >
        <div className="glass-panel border border-white/10 rounded-3xl px-6 py-5 bg-white/5 backdrop-blur-2xl">
          <div className="flex flex-col gap-4">
            <div className="flex items-start justify-between gap-6">
              <div>
                <div className="font-display text-xs tracking-[0.35em] uppercase text-white/70">
                  Cookies & приватность
                </div>
                <div className="mt-2 text-sm text-white/70 leading-relaxed">
                  Мы используем необходимые cookies для работы сайта. По желанию вы можете разрешить аналитические и
                  маркетинговые cookies. Подробнее — в{' '}
                  <Link href="/privacy" className="text-[#ddb7ff] hover:text-white underline underline-offset-4">
                    Политике обработки ПДн
                  </Link>
                  .
                </div>
              </div>
              <button
                type="button"
                onClick={closeMinimal}
                className="text-white/40 hover:text-white/70 text-xs font-mono tracking-widest"
                aria-label="Закрыть"
              >
                Закрыть
              </button>
            </div>

            {!openSettings ? (
              <div className="flex flex-col sm:flex-row gap-3 sm:items-center sm:justify-end">
                <button
                  type="button"
                  onClick={rejectNonEssential}
                  className="px-5 py-2.5 rounded-2xl border border-white/15 bg-white/5 hover:bg-white/10 text-xs font-mono tracking-widest text-white/80"
                >
                  Отклонить
                </button>
                <button
                  type="button"
                  onClick={() => setOpenSettings(true)}
                  className="px-5 py-2.5 rounded-2xl border border-white/15 bg-transparent hover:bg-white/5 text-xs font-mono tracking-widest text-white/80"
                >
                  Настроить
                </button>
                <button
                  type="button"
                  onClick={acceptAll}
                  className="px-5 py-2.5 rounded-2xl bg-[#ddb7ff] hover:bg-white text-black text-xs font-mono tracking-widest"
                >
                  Принять все
                </button>
              </div>
            ) : (
              <div className="rounded-2xl border border-white/10 bg-black/30 px-5 py-4">
                <div className="text-[10px] font-mono tracking-[0.25em] uppercase text-white/50 mb-3">
                  Настройки cookies
                </div>

                <div className="space-y-3">
                  <label className="flex items-center justify-between gap-4 text-sm text-white/70">
                    <span>
                      Необходимые <span className="text-white/40 text-xs">(всегда включены)</span>
                    </span>
                    <input type="checkbox" checked readOnly className="accent-[#ddb7ff]" />
                  </label>
                  <label className="flex items-center justify-between gap-4 text-sm text-white/70">
                    <span>Аналитические</span>
                    <input
                      type="checkbox"
                      checked={analytics}
                      onChange={(e) => setAnalytics(e.target.checked)}
                      className="accent-[#ddb7ff]"
                    />
                  </label>
                  <label className="flex items-center justify-between gap-4 text-sm text-white/70">
                    <span>Маркетинговые</span>
                    <input
                      type="checkbox"
                      checked={marketing}
                      onChange={(e) => setMarketing(e.target.checked)}
                      className="accent-[#ddb7ff]"
                    />
                  </label>
                </div>

                <div className="mt-4 flex flex-col sm:flex-row gap-3 sm:items-center sm:justify-end">
                  <button
                    type="button"
                    onClick={() => setOpenSettings(false)}
                    className="px-5 py-2.5 rounded-2xl border border-white/15 bg-white/5 hover:bg-white/10 text-xs font-mono tracking-widest text-white/80"
                  >
                    Назад
                  </button>
                  <button
                    type="button"
                    onClick={saveSettings}
                    className="px-5 py-2.5 rounded-2xl bg-[#ddb7ff] hover:bg-white text-black text-xs font-mono tracking-widest"
                  >
                    Сохранить
                  </button>
                </div>
              </div>
            )}
          </div>
        </div>
      </motion.div>
    </AnimatePresence>
  );
}

