'use client';

import React, { useEffect, useMemo, useRef, useState } from 'react';
import Link from 'next/link';
import { motion, useScroll, useSpring, useTransform } from 'framer-motion';
import { ApiClient } from '../lib/api';
import { useToast } from '../components/Toast';
import {
  Activity,
  AlertTriangle,
  ChevronRight,
  Code2,
  Key,
  Lock,
  Network,
  Radio,
  RefreshCw,
  Shield,
  Target,
  Users,
  Zap,
  Headset,
} from 'lucide-react';

// Deterministic pseudo-random for SSR/CSR hydration stability (avoid Math.random() during render).
function mulberry32(seed: number) {
  return function () {
    let t = (seed += 0x6D2B79F5);
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function scrollToId(id: string) {
  if (typeof window === 'undefined') return;
  const el = document.getElementById(id);
  if (!el) return;
  el.scrollIntoView({ behavior: 'smooth', block: 'start' });
}

const NAV_ITEMS = [
  { label: 'Системы', id: 'systems' },
  { label: 'Карта угроз', id: 'threat-map' },
  { label: 'FAQ', id: 'faq' },
  { label: 'Пилот', id: 'trust' },
  { label: 'Контакт', id: 'contact' },
] as const;

const Terminal = () => {
  const bars = useMemo(() => {
    const rnd = mulberry32(0xA3E61);
    return Array.from({ length: 12 }, (_, i) => ({
      key: i,
      duration: 2 + rnd(), // stable 2..3
      delay: i * 0.1,
    }));
  }, []);

  return (
    <div className="glass-panel rounded-3xl overflow-hidden border-white/5 w-full max-w-4xl mx-auto shadow-4xl">
      <div className="bg-surface-dim/80 px-6 py-4 border-b border-white/5 flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="flex gap-1.5">
            <div className="w-3 h-3 rounded-full bg-red-500/40" />
            <div className="w-3 h-3 rounded-full bg-yellow-500/40" />
            <div className="w-3 h-3 rounded-full bg-green-500/40" />
          </div>
          <span className="ml-4 font-mono text-[10px] text-on-surface-variant tracking-[0.3em] opacity-50">
            СИСТЕМНЫЙ_ПРОТОКОЛ: АДАПТАЦИЯ В РЕАЛЬНОМ ВРЕМЕНИ
          </span>
        </div>
        <div className="flex items-center gap-4">
          <div className="text-[10px] font-mono text-primary-neon animate-pulse flex items-center gap-2 tracking-widest font-bold">
            <span className="w-1.5 h-1.5 rounded-full bg-primary-neon shadow-[0_0_10px_#8a2be2]" /> ПРЯМОЙ
            КАНАЛ
          </div>
        </div>
      </div>
      <div className="grid grid-cols-1 lg:grid-cols-12">
        <div className="lg:col-span-8 p-8 font-mono text-[13px] text-primary-soft/80 bg-black/40 min-h-[400px]">
          <div className="mb-6 flex items-center justify-between border-b border-white/5 pb-4">
            <div className="text-outline/50 uppercase tracking-widest text-[9px] font-bold">
              // NOIR_CORE_V4.2.0_INIT
            </div>
            <div className="flex items-center gap-2">
              <span className="w-1.5 h-1.5 rounded-full bg-primary-neon animate-ping" />
              <span className="text-[9px] text-primary-neon/60 font-black uppercase">Active</span>
            </div>
          </div>
          <div className="space-y-2 opacity-90">
            <p className="text-outline/50 text-[11px]">[04:21:09] Booting AEGIS Neural Mesh...</p>
            <p>
              <span className="text-secondary-neon">import</span> {'{ ImmunityCore }'}{' '}
              <span className="text-secondary-neon">from</span>{' '}
              <span className="text-primary-soft">'@aegis/core'</span>;
            </p>
            <p>
              <span className="text-secondary-neon">import</span> {'{ NeuralAdapt }'}{' '}
              <span className="text-secondary-neon">from</span>{' '}
              <span className="text-primary-soft">'@aegis/ai'</span>;
            </p>
            <div className="h-4" />
            <p>
              <span className="text-secondary-neon">const</span> node ={' '}
              <span className="text-secondary-neon">new</span> ImmunityCore({'{'}
            </p>
            <p className="ml-6">
              mode: <span className="text-primary-soft">'autonomous'</span>,
            </p>
            <p className="ml-6">
              sensitivity: <span className="text-primary-soft">0.9997</span>,
            </p>
            <p className="ml-6">
              autoHeal: <span className="text-secondary-neon">true</span>,
            </p>
            <p className="ml-6">
              latency_threshold: <span className="text-primary-soft">'5ms'</span>
            </p>
            <p>{'})'};</p>
            <div className="h-4" />
            <motion.div initial={{ opacity: 0 }} whileInView={{ opacity: 1 }} className="space-y-1">
              <p className="text-primary-neon flex items-center gap-2">
                <span className="text-[10px]">▶</span>
                <span>Anomalous payload detected: [Sector 7G]</span>
              </p>
              <p className="text-white/70 flex items-center gap-2">
                <span className="text-[10px]">▶</span>
                <span>Analyzing behavioral sequence...</span>
              </p>
              <p className="text-secondary-neon flex items-center gap-2">
                <span className="text-[10px]">▶</span>
                <span>Applying polymorphic patch [PX-992]</span>
              </p>
              <p className="text-primary-neon flex items-center gap-2">
                <span className="text-[10px]">▶</span>
                <span>Neutralized. 0ms downtime recorded.</span>
              </p>
            </motion.div>
          </div>
          <div className="mt-8 flex items-center gap-2">
            <span className="text-primary-neon font-bold">$</span>
            <motion.span
              animate={{ opacity: [0, 1] }}
              transition={{ repeat: Infinity, duration: 0.8 }}
              className="w-2 h-5 bg-primary-neon/40"
            />
          </div>
        </div>
        <div className="lg:col-span-4 p-8 border-l border-white/5 bg-surface-dim/20">
          <div className="flex justify-between items-center mb-10">
            <h4 className="font-display text-sm tracking-widest uppercase opacity-60">Панель управления</h4>
            <Activity className="text-primary-neon/40 w-5 h-5" />
          </div>
          <div className="space-y-10">
            <div>
              <div className="flex justify-between font-display text-[10px] mb-3 text-on-surface-variant tracking-[0.25em] opacity-60">
                <span>СИСТЕМНАЯ ЭНТРОПИЯ</span>
                <span className="text-primary-neon">0.02 Δ</span>
              </div>
              <div className="h-12 w-full flex items-end gap-1">
                {bars.map((b) => (
                  <motion.div
                    key={b.key}
                    animate={{ height: [10, 40, 20, 35, 10] }}
                    transition={{ repeat: Infinity, duration: b.duration, delay: b.delay }}
                    className="flex-1 bg-primary-neon/20 border-t border-primary-neon/40"
                  />
                ))}
              </div>
            </div>
            <div className="space-y-8">
              <div className="group">
                <div className="flex justify-between font-display text-[10px] mb-3 text-on-surface-variant tracking-[0.25em]">
                  <span>АУДИТ БЕЗОПАСНОСТИ</span>
                  <span className="text-primary-neon font-mono">92.4%</span>
                </div>
                <div className="h-1 bg-white/5 rounded-full overflow-hidden">
                  <motion.div
                    initial={{ width: 0 }}
                    whileInView={{ width: '92.4%' }}
                    transition={{ duration: 2, ease: 'easeOut' }}
                    className="h-full bg-primary-neon shadow-[0_0_15px_#8a2be2]"
                  />
                </div>
              </div>
              <div className="group">
                <div className="flex justify-between font-display text-[10px] mb-3 text-on-surface-variant tracking-[0.25em]">
                  <span>РАЗВЕРТЫВАНИЕ</span>
                  <span className="text-primary-neon font-mono">78.1%</span>
                </div>
                <div className="h-1 bg-white/5 rounded-full overflow-hidden">
                  <motion.div
                    initial={{ width: 0 }}
                    whileInView={{ width: '78.1%' }}
                    transition={{ duration: 2, ease: 'easeOut', delay: 0.5 }}
                    className="h-full bg-primary-neon shadow-[0_0_15px_#8a2be2]"
                  />
                </div>
              </div>
            </div>
          </div>
          <button className="w-full mt-12 bg-primary-neon/10 border border-primary-neon/20 text-primary-neon py-4 rounded-xl font-display text-[10px] tracking-[0.4em] uppercase hover:bg-primary-neon/20 transition-all">
            Запустить симуляцию
          </button>
        </div>
      </div>
    </div>
  );
};

type BuiltPhase = {
  number: string;
  title: string;
  description: React.ReactNode;
};

const BUILT_PHASES: BuiltPhase[] = [
  {
    number: 'PHASE 1',
    title: 'Zero-Trust Foundation',
    description:
      'Полноценная Zero-Trust архитектура: KeyProvider, mTLS, Prompt Guard, Rate Limiting и неизменяемый Audit Trail. Ни одна операция не выполняется без проверки.',
  },
  {
    number: 'PHASE 2',
    title: 'Self-Healing & Defense',
    description: (
      <>
        Система, которая не просто реагирует, а <strong>самовосстанавливается</strong>. Healing Orchestrator с формальной верификацией, Honeypots и Distributed Oracle. Угрозы нейтрализуются до того, как вы о них узнаете.
      </>
    ),
  },
  {
    number: 'PHASE 3',
    title: 'Federation & Deception',
    description:
      'Распределённая сеть нод с P2P-синхронизацией и Moving Target Defense. Поверхность атаки постоянно меняется. Advanced Deception автоматически разворачивает ловушки при обнаружении угрозы.',
  },
  {
    number: 'PHASE 4',
    title: 'Hardened Verification',
    description:
      'Формальная верификация на уровне AST + Taint Tracking. Обнаруживает опасный код даже при хитром форматировании. Все секреты хранятся в HSM/Vault. E2E-тесты на каждом этапе.',
  },
  {
    number: 'PHASE 5',
    title: 'Autonomous Evolution',
    description: (
      <>
        Self-Healing 2.0 с частичной автономией. Honeypots 2.0 с реальным Firecracker. Raft 2.0 с log replication. Система не просто защищает — она <strong>эволюционирует</strong> вместе с угрозами.
      </>
    ),
  },
];

function PhaseCard({ phase }: { phase: BuiltPhase }) {
  const [hovered, setHovered] = useState(false);
  const [pinned, setPinned] = useState(false);
  const [isMobile, setIsMobile] = useState(false);

  useEffect(() => {
    const checkMobile = () => setIsMobile(window.innerWidth < 768);
    checkMobile();
    window.addEventListener('resize', checkMobile);
    return () => window.removeEventListener('resize', checkMobile);
  }, []);

  const flipped = hovered || pinned;

  return (
    <div
      className="relative h-[280px] cursor-pointer select-none outline-none focus-visible:ring-2 focus-visible:ring-primary-neon/60 rounded-3xl"
      onMouseEnter={() => !isMobile && setHovered(true)}
      onMouseLeave={() => {
        if (!isMobile) {
          setHovered(false);
          setPinned(false);
        }
      }}
      onClick={() => {
        if (isMobile) {
          setPinned(!pinned);
        } else {
          setPinned((p) => !p);
        }
      }}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          setPinned((p) => !p);
        }
      }}
      role="button"
      tabIndex={0}
      aria-pressed={flipped}
      aria-label={`${phase.number}: ${phase.title}. ${flipped ? 'Описание открыто' : 'Показать описание'}`}
    >
      <div className="absolute inset-0 [perspective:1200px]">
        <motion.div
          className="relative h-full w-full"
          initial={false}
          animate={{ rotateY: flipped ? 180 : 0 }}
          transition={{ duration: 0.55, ease: [0.4, 0, 0.2, 1] }}
          style={{ transformStyle: 'preserve-3d' }}
        >
          <div className="absolute inset-0 glass-card rounded-3xl border border-white/10 p-6 md:p-8 flex flex-col justify-between [backface-visibility:hidden] [transform:translateZ(0.1px)]">
            <div>
              <div className="font-display text-[#00F5A3] text-xs tracking-[0.25em] mb-3 font-bold">{phase.number}</div>
              <h3 className="font-display text-2xl md:text-3xl font-bold tracking-tight text-white leading-tight">{phase.title}</h3>
            </div>
            <div className="text-white/50 text-xs font-display tracking-widest uppercase">
              Наведите курсор или нажмите
            </div>
          </div>
          <div
            className="absolute inset-0 glass-card rounded-3xl border border-white/10 p-6 md:p-8 flex items-center justify-center [backface-visibility:hidden]"
            style={{ transform: 'rotateY(180deg) translateZ(1px)' }}
          >
            <p className="text-white/90 text-[15px] md:text-base leading-relaxed font-sans text-left">{phase.description}</p>
          </div>
        </motion.div>
      </div>
    </div>
  );
}

type KeyFeature = {
  icon: React.ReactNode;
  title: string;
  description: string;
};

const features: KeyFeature[] = [
  {
    icon: <Network className="w-8 h-8" />,
    title: 'Federation Layer',
    description:
      'P2P синхронизация, дельта-обмен, Merkle Root, mTLS. Ноды автоматически находят друг друга.',
  },
  {
    icon: <Shield className="w-8 h-8" />,
    title: 'Moving Target Defense',
    description:
      'Постоянная мутация fingerprint, ротация портов, динамические honeypots. Поверхность атаки всегда меняется.',
  },
  {
    icon: <Target className="w-8 h-8" />,
    title: 'Advanced Deception',
    description:
      'Автономное развёртывание ловушек. При срабатывании Canary — автоматически создаются новые.',
  },
  {
    icon: <RefreshCw className="w-8 h-8" />,
    title: 'Self-Healing 2.0',
    description: 'Частичная автономия. Low/Medium патчи применяются автоматически при низком риске.',
  },
  {
    icon: <Code2 className="w-8 h-8" />,
    title: 'AST + Taint Verification',
    description:
      'Формальная верификация через AST и taint tracking. Обнаруживает опасный код даже при хитром форматировании.',
  },
  {
    icon: <Key className="w-8 h-8" />,
    title: 'HSM / Vault',
    description: 'Хранение всех секретов в HashiCorp Vault. Zero-Trust на уровне ключей.',
  },
  {
    icon: <Users className="w-8 h-8" />,
    title: 'Raft 2.0',
    description:
      'Реальный Raft с log replication, state machine и majority commit. Полноценный распределённый консенсус.',
  },
  {
    icon: <Lock className="w-8 h-8" />,
    title: 'Zero-Trust на всех уровнях',
    description:
      'mTLS, Prompt Guard, Rate Limiting, Audit Trail, HITL. Ни одна операция не выполняется без проверки.',
  },
];

function FeatureCard({ feature }: { feature: KeyFeature }) {
  return (
    <div className="glass-card rounded-3xl p-6 md:p-8 hover:border-[#00F5A3]/50 transition-all group">
      <div className="text-[#00F5A3] mb-6 group-hover:scale-110 transition-transform">{feature.icon}</div>
      <h3 className="text-xl md:text-2xl font-bold tracking-tight mb-4">{feature.title}</h3>
      <p className="text-white/70 text-sm md:text-[15px] leading-relaxed">{feature.description}</p>
    </div>
  );
}

const KeyCapabilitiesSection = () => (
  <section className="py-20 border-t border-white/10">
    <div className="max-w-6xl mx-auto px-6">
      <div className="text-center mb-16">
        <div className="text-[#00F5A3] text-sm tracking-[3px] mb-4">ТЕХНОЛОГИЧЕСКОЕ ПРЕВОСХОДСТВО</div>
        <h2 className="text-5xl font-bold tracking-tighter">Ключевые возможности</h2>
        <p className="mt-4 text-xl text-white/70 max-w-2xl mx-auto">
          Всё, что нужно для настоящей автономной защиты
        </p>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-6">
        {features.map((feature, index) => (
          <FeatureCard key={index} feature={feature} />
        ))}
      </div>
    </div>
  </section>
);

const PilotSection = () => (
  <section id="pilot" className="py-20 border-t border-white/10 bg-black/60">
    <div className="max-w-4xl mx-auto px-6 text-center">
      <div className="text-[#00F5A3] text-sm tracking-[3px] mb-4">ГОТОВЫ К ПИЛОТУ</div>
      
      <h2 className="text-5xl font-bold tracking-tighter mb-6">
        Хотите увидеть AEGIS в действии?
      </h2>
      
      <p className="text-xl text-white/70 max-w-2xl mx-auto mb-10">
        Оставьте заявку, и мы проведём персональную демонстрацию + поможем с развёртыванием пилота
      </p>

      {/* Форма заявки */}
      <div className="glass-card rounded-3xl p-10 max-w-xl mx-auto">
        <form className="space-y-6">
          <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
            <div>
              <label className="block text-sm text-white/70 mb-2">Имя</label>
              <input 
                type="text" 
                className="w-full bg-black/40 border border-white/20 rounded-xl px-4 py-3 text-white placeholder-white/50 focus:border-[#00F5A3] outline-none"
                placeholder="Иван Иванов"
              />
            </div>
            <div>
              <label className="block text-sm text-white/70 mb-2">Компания</label>
              <input 
                type="text" 
                className="w-full bg-black/40 border border-white/20 rounded-xl px-4 py-3 text-white placeholder-white/50 focus:border-[#00F5A3] outline-none"
                placeholder="Название компании"
              />
            </div>
          </div>

          <div>
            <label className="block text-sm text-white/70 mb-2">Email</label>
            <input 
              type="email" 
              className="w-full bg-black/40 border border-white/20 rounded-xl px-4 py-3 text-white placeholder-white/50 focus:border-[#00F5A3] outline-none"
              placeholder="you@company.com"
            />
          </div>

          <div>
            <label className="block text-sm text-white/70 mb-2">Что вас интересует?</label>
            <textarea 
              className="w-full bg-black/40 border border-white/20 rounded-xl px-4 py-3 text-white placeholder-white/50 focus:border-[#00F5A3] outline-none h-28 resize-y"
              placeholder="Хотим протестировать на нашей инфраструктуре..."
            />
          </div>

          <button 
            type="submit"
            className="w-full bg-[#00F5A3] hover:bg-[#00E090] text-black font-semibold py-4 rounded-2xl transition-all active:scale-[0.985]"
          >
            Отправить заявку на пилот
          </button>
        </form>

        <p className="text-xs text-white/50 mt-6">
          Мы свяжемся с вами в течение 24 часов
        </p>
      </div>

      {/* Trust-маркеры */}
      <div className="mt-12 flex flex-wrap justify-center gap-x-8 gap-y-4 text-white/60 text-sm">
        <div className="flex items-center gap-2">
          <div className="w-2 h-2 bg-[#00F5A3] rounded-full" /> Rust
        </div>
        <div className="flex items-center gap-2">
          <div className="w-2 h-2 bg-[#00F5A3] rounded-full" /> gRPC
        </div>
        <div className="flex items-center gap-2">
          <div className="w-2 h-2 bg-[#00F5A3] rounded-full" /> Qdrant
        </div>
        <div className="flex items-center gap-2">
          <div className="w-2 h-2 bg-[#00F5A3] rounded-full" /> 11 источников Threat Intel
        </div>
      </div>
    </div>
  </section>
);

const WhatWeBuiltSection = () => (
  <section id="phases-built" className="py-20 border-t border-white/10 bg-black/40">
    <div className="max-w-6xl mx-auto px-6">
      <div className="text-center mb-16">
        <div className="font-display text-[#00F5A3] text-sm tracking-[0.35em] mb-4 font-bold">ФАЗЫ 1–5 ЗАВЕРШЕНЫ</div>
        <h2 className="font-display text-4xl md:text-5xl font-bold tracking-tighter text-white">Что мы построили</h2>
        <p className="mt-4 text-lg md:text-xl text-white/70 max-w-2xl mx-auto font-sans leading-relaxed">
          Полноценная автономная иммунная система с Zero-Trust на всех уровнях
        </p>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-6">
        {BUILT_PHASES.map((phase) => (
          <PhaseCard key={phase.number} phase={phase} />
        ))}
      </div>
    </div>
  </section>
);

const ThreatMap = () => {
  const points = useMemo(
    () => [
      { top: '30%', left: '25%', label: 'NA_CORE_1' },
      { top: '45%', left: '48%', label: 'EU_NODE_7' },
      { top: '65%', left: '75%', label: 'ASIA_MESH_4' },
      { top: '25%', left: '80%', label: 'RU_UPLINK_2' },
      { top: '75%', left: '30%', label: 'SA_LINK_9' },
      { top: '55%', left: '15%', label: 'PAC_HUB_3' },
    ],
    []
  );

  const ringDelays = useMemo(() => {
    const rnd = mulberry32(0xDE1A7);
    return points.map(() => rnd() * 1.5); // stable 0..1.5
  }, [points]);

  return (
    <section id="threat-map" className="px-6 py-40 max-w-7xl mx-auto overflow-hidden">
      <div className="alche-border rounded-[3rem] p-12 lg:p-24 relative overflow-hidden bg-white/[0.02]">
        <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[120%] h-[120%] bg-primary-neon/5 blur-[120px] rounded-full pointer-events-none" />

        <div className="relative z-10 grid grid-cols-1 lg:grid-cols-2 gap-16 items-center">
          <div>
            <span className="text-secondary-neon font-display text-xs tracking-[0.5em] uppercase mb-8 block font-bold">
              Глобальная разведка
            </span>
            <h2 className="font-display text-5xl md:text-6xl font-bold uppercase tracking-tighter mb-10 leading-tight">
              Карта распределенных узлов AEGIS
            </h2>
            <div className="space-y-6">
              {[
                { label: 'Нейтрализовано активных угроз', value: '1.2M+' },
                { label: 'Средняя задержка', value: '14мс' },
                { label: 'Активных нейро-реплик', value: '842' },
              ].map((stat, i) => (
                <motion.div
                  key={i}
                  initial={{ opacity: 0, x: -20 }}
                  whileInView={{ opacity: 1, x: 0 }}
                  transition={{ delay: i * 0.1 }}
                  className="flex items-center gap-4 border-b border-white/5 pb-4 group"
                >
                  <div className="w-10 h-10 rounded-lg bg-primary-neon/5 flex items-center justify-center group-hover:bg-primary-neon/20 transition-all">
                    <Radio className="w-5 h-5 text-primary-neon" />
                  </div>
                  <span className="text-on-surface-variant font-sans opacity-60 flex-1">{stat.label}</span>
                  <span className="text-primary-neon font-display font-bold text-xl">{stat.value}</span>
                </motion.div>
              ))}
            </div>

            <div className="mt-12 flex items-center gap-4 p-4 rounded-2xl bg-white/5 border border-white/5">
              <div className="w-2 h-2 rounded-full bg-primary-neon animate-pulse" />
              <div className="text-[10px] font-mono text-primary-soft uppercase tracking-widest leading-tight">
                Обнаружена попытка внедрения в секторе RU-77 <br />
                <span className="opacity-40">Статус: Автоматическая блокировка...</span>
              </div>
            </div>
          </div>

          <div className="relative aspect-square rounded-[2rem] border border-white/10 bg-black/40 overflow-hidden shadow-2xl flex items-center justify-center group">
            <div className="relative w-full h-full flex items-center justify-center">
              <div className="absolute w-[70%] h-[70%] bg-primary-neon/10 blur-[80px] rounded-full" />

              <motion.div
                animate={{ rotate: 360 }}
                transition={{ duration: 120, repeat: Infinity, ease: 'linear' }}
                className="relative w-[75%] h-[75%] rounded-full border border-white/10 shadow-[inset_0_0_50px_rgba(138,43,226,0.2)] flex items-center justify-center"
              >
                <svg
                  viewBox="0 0 100 100"
                  className="w-[85%] h-[85%] text-primary-neon opacity-20"
                  fill="currentColor"
                >
                  <path
                    d="M20,40 Q25,35 30,40 T40,45 T50,40 T60,45 T70,40 T80,45 M15,55 Q20,50 25,55 T35,60 T45,55 T55,60 T65,55 T75,60 T85,55"
                    stroke="currentColor"
                    strokeWidth="0.5"
                    fill="none"
                  />
                  <path
                    d="M25,25 Q30,20 35,25 T45,30 T55,20 T65,25 T75,20 L80,30 L70,40 L60,35 L50,45 L40,40 L30,45 Z"
                    opacity="0.3"
                  />
                  <path
                    d="M20,60 Q25,55 35,60 T45,65 T55,60 T65,65 T75,60 L80,70 L70,80 L60,75 L50,85 L40,80 L30,85 Z"
                    opacity="0.3"
                  />
                </svg>

                <div className="absolute inset-0 rounded-full border border-white/5 opacity-40">
                  <div className="absolute top-1/2 left-0 w-full h-px bg-white/10" />
                  <div className="absolute top-0 left-1/2 w-px h-full bg-white/10" />
                  <div className="absolute inset-0 rounded-full border border-dashed border-white/10 animate-[spin_60s_linear_infinite]" />
                </div>
              </motion.div>

              <div className="absolute inset-0">
                {points.map((p, i) => (
                  <motion.div
                    key={i}
                    style={{ top: p.top, left: p.left }}
                    className="absolute"
                    initial={{ opacity: 0, scale: 0 }}
                    whileInView={{ opacity: 1, scale: 1 }}
                    transition={{ delay: i * 0.2 }}
                  >
                    <div className="relative">
                      <motion.div
                        animate={{ scale: [1, 2, 1], opacity: [1, 0, 1] }}
                        transition={{ duration: 2, repeat: Infinity, delay: ringDelays[i] ?? 0 }}
                        className="absolute inset-[-8px] border border-primary-neon rounded-full"
                      />
                      <div className="w-2 h-2 bg-primary-neon rounded-full shadow-[0_0_10px_#8a2be2]" />

                      <div className="absolute top-4 left-4 bg-black/60 backdrop-blur-md px-2 py-1 rounded border border-white/10 whitespace-nowrap">
                        <span className="text-[8px] font-mono text-primary-soft uppercase">{p.label}</span>
                      </div>
                    </div>
                  </motion.div>
                ))}
              </div>

              <div className="absolute inset-0 rounded-full shadow-[0_0_100px_rgba(138,43,226,0.1)_inset,0_0_40px_rgba(96,165,250,0.1)] pointer-events-none" />
            </div>

            <div className="absolute bottom-10 left-10 right-10 flex justify-between items-center bg-white/5 backdrop-blur-xl p-4 rounded-2xl border border-white/10 border-t-white/20">
              <div className="flex items-center gap-3">
                <div className="flex gap-1">
                  <div className="w-1 h-3 bg-primary-neon/40 rounded-full" />
                  <motion.div
                    animate={{ height: [8, 16, 8] }}
                    transition={{ repeat: Infinity, duration: 1 }}
                    className="w-1 h-4 bg-primary-neon rounded-full"
                  />
                  <div className="w-1 h-2 bg-primary-neon/40 rounded-full" />
                </div>
                <span className="text-[9px] font-mono uppercase tracking-widest opacity-60">Анализ трафика</span>
              </div>
              <div className="text-[14px] font-display font-bold text-primary-neon tracking-tighter">
                ACTIVE_IMMUNITY: ON
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
};

const AtmosphericOrbs = () => {
  return (
    <div className="fixed inset-0 pointer-events-none z-[-2] overflow-hidden">
      <motion.div
        animate={{
          x: [0, 100, -50, 0],
          y: [0, -50, 50, 0],
          scale: [1, 1.2, 0.9, 1],
          opacity: [0.1, 0.2, 0.1],
        }}
        transition={{ duration: 25, repeat: Infinity, ease: 'linear' }}
        className="absolute top-[-10%] left-[-10%] w-[60%] h-[60%] rounded-full bg-primary-neon/10 blur-[150px]"
      />
      <motion.div
        animate={{
          x: [0, -80, 100, 0],
          y: [0, 100, -50, 0],
          scale: [1, 0.8, 1.1, 1],
          opacity: [0.05, 0.15, 0.05],
        }}
        transition={{ duration: 30, repeat: Infinity, ease: 'linear' }}
        className="absolute bottom-[-10%] right-[-10%] w-[50%] h-[50%] rounded-full bg-secondary-neon/10 blur-[130px]"
      />
      <motion.div
        animate={{
          opacity: [0, 0.1, 0],
          scale: [0.8, 1.2, 0.8],
        }}
        transition={{ duration: 15, repeat: Infinity, ease: 'easeInOut' }}
        className="absolute top-[30%] left-[40%] w-[40%] h-[40%] rounded-full bg-blue-500/5 blur-[120px]"
      />
    </div>
  );
};

const BackgroundParticles = () => {
  const particles = useMemo(() => {
    const rnd = mulberry32(0xBADC0DE);
    return Array.from({ length: 40 }, (_, i) => {
      const x = rnd() * 100;
      const y = rnd() * 100;
      const opacity = rnd() * 0.4;
      const scale = rnd() * 0.5 + 0.5;
      const dirY = rnd() > 0.5 ? '-' : '+';
      const dirX = rnd() > 0.5 ? '-' : '+';
      const duration = 15 + rnd() * 30;
      return { i, x, y, opacity, scale, dirY, dirX, duration };
    });
  }, []);

  return (
    <div className="fixed inset-0 pointer-events-none z-[-1] overflow-hidden">
      {particles.map((p) => (
        <motion.div
          key={p.i}
          initial={{
            x: p.x + '%',
            y: p.y + '%',
            opacity: p.opacity,
            scale: p.scale,
          }}
          animate={{
            y: [null, `${p.dirY}50%`],
            x: [null, `${p.dirX}20%`],
            opacity: [0.1, 0.5, 0.1],
          }}
          transition={{
            duration: p.duration,
            repeat: Infinity,
            ease: 'linear',
          }}
          className={`absolute rounded-full bg-primary-neon/30 ${p.i % 3 === 0 ? 'w-[2px] h-[2px]' : 'w-px h-8 bg-gradient-to-b from-primary-neon/40 to-transparent'}`}
        />
      ))}
    </div>
  );
};

const AnimatedSVGBackground = () => {
  const pulsePaths = useMemo(() => {
    const rnd = mulberry32(0x51C0FFEE);
    return Array.from({ length: 15 }, (_, i) => ({
      i,
      duration: 6 + rnd() * 4,
      delay: i * 0.8,
    }));
  }, []);

  const micros = useMemo(() => {
    const rnd = mulberry32(0xC0FFEE12);
    return Array.from({ length: 60 }, (_, i) => ({
      i,
      cx: Math.round(rnd() * 1000),
      cy: Math.round(rnd() * 1000),
      duration: 3 + rnd() * 5,
      delay: rnd() * 10,
    }));
  }, []);

  return (
    <div className="fixed inset-0 z-[-2] pointer-events-none bg-[#030014] overflow-hidden">
      <svg className="w-full h-full opacity-40" viewBox="0 0 1000 1000" preserveAspectRatio="xMidYMid slice">
        <motion.g
          animate={{ rotate: 180 }}
          transition={{ duration: 180, repeat: Infinity, ease: 'linear' }}
          style={{ originX: '500px', originY: '500px' }}
        >
          {[...Array(20)].map((_, i) => (
            <g key={i}>
              <line x1="0" y1={i * 50} x2="1000" y2={i * 50} stroke="#a855f7" strokeWidth="0.2" strokeOpacity="0.1" />
              <line x1={i * 50} y1="0" x2={i * 50} y2="1000" stroke="#a855f7" strokeWidth="0.2" strokeOpacity="0.1" />
            </g>
          ))}
        </motion.g>

        {[...Array(6)].map((_, i) => (
          <motion.rect
            key={`rect-${i}`}
            x={200 + i * 80}
            y={200 + i * 80}
            width="300"
            height="300"
            fill="none"
            stroke="#60a5fa"
            strokeWidth="0.3"
            strokeOpacity="0.05"
            animate={{
              rotate: [0, 90, 180, 270, 360],
              scale: [0.8, 1.2, 0.8],
              opacity: [0.02, 0.1, 0.02],
            }}
            transition={{
              duration: 30 + i * 10,
              repeat: Infinity,
              ease: 'linear',
            }}
            style={{ originX: `${200 + i * 80 + 150}px`, originY: `${200 + i * 80 + 150}px` }}
          />
        ))}

        {pulsePaths.map((p) => {
          const x1 = (p.i * 77) % 1000;
          const y1 = (p.i * 123) % 1000;
          const x2 = x1 + 150;
          const y2 = y1 + 100;
          return (
            <motion.path
              key={`pulse-${p.i}`}
              d={`M ${x1} ${y1} L ${x2} ${y2}`}
              stroke="#a855f7"
              strokeWidth="0.5"
              fill="none"
              initial={{ pathLength: 0, opacity: 0 }}
              animate={{
                pathLength: [0, 1, 1],
                opacity: [0, 0.2, 0],
              }}
              transition={{
                duration: p.duration,
                repeat: Infinity,
                delay: p.delay,
              }}
            />
          );
        })}

        {micros.map((m) => (
          <motion.circle
            key={`micro-${m.i}`}
            cx={m.cx}
            cy={m.cy}
            r="0.4"
            fill="white"
            initial={{ opacity: 0 }}
            animate={{ opacity: [0, 0.3, 0] }}
            transition={{
              duration: m.duration,
              repeat: Infinity,
              delay: m.delay,
            }}
          />
        ))}

        <circle cx="20%" cy="30%" r="200" fill="url(#hero-grad)" opacity="0.1" />
        <circle cx="80%" cy="70%" r="250" fill="url(#hero-grad)" opacity="0.05" />

        <defs>
          <radialGradient id="hero-grad">
            <stop offset="0%" stopColor="#8a2be2" />
            <stop offset="100%" stopColor="transparent" />
          </radialGradient>
        </defs>
      </svg>

      <div className="absolute inset-0 bg-[radial-gradient(circle_at_50%_50%,rgba(3,0,20,0.4),rgba(3,0,20,1))]" />
      <div className="absolute top-0 left-0 w-full h-full opacity-20 pointer-events-none mix-blend-screen bg-[conic-gradient(from_0deg_at_50%_50%,rgba(168,85,247,0.05),transparent,rgba(96,165,250,0.05),transparent)] animate-[spin_60s_linear_infinite]" />
    </div>
  );
};

const ThreatTicker = () => {
  const threats = [
    'SQL Injection blocked [Sector RU-1]',
    'DDoS mitigation active [Global Mesh]',
    'Z-Day vulnerability patched [Node 742]',
    'Neural adaptation sync: 100%',
    'Anomaly detected in encrypted tunnel [EU-9]',
    'Polymorphic filter update applied',
  ];

  return (
    <div className="w-full bg-primary-neon/5 border-y border-white/5 py-3 overflow-hidden whitespace-nowrap relative">
      <motion.div
        animate={{ x: [0, -1000] }}
        transition={{ duration: 30, repeat: Infinity, ease: 'linear' }}
        className="flex gap-20 items-center"
      >
        {[...threats, ...threats, ...threats].map((threat, i) => (
          <div key={i} className="flex items-center gap-3">
            <div className="w-1 h-1 rounded-full bg-primary-neon" />
            <span className="font-mono text-[10px] uppercase tracking-[0.2em] text-primary-soft/90 opacity-70">
              {threat}
            </span>
          </div>
        ))}
      </motion.div>
    </div>
  );
};

const DefenseTiers = () => {
  const tiers = [
    {
      name: 'Starter',
      level: 'Бесплатно',
      price: '0 ₽',
      desc: 'Стартовая защита для небольшого контура и знакомства с AEGIS.',
      features: ['1 сервер', 'Базовые агенты', 'Ограниченный Threat Hunter', '1 000 событий / день'],
      cta: 'Начать бесплатно',
      intent: 'free',
      accent: 'text-white',
    },
    {
      name: 'Professional',
      level: '199 000 ₽ / месяц',
      price: '199 000 ₽',
      desc: 'Пакет для SOC/IT службы: полный рой ReAct++ агентов и управляемый GOD MODE.',
      features: ['До 10 серверов', 'Полный рой ReAct++ агентов', 'GOD MODE + HITL', '100 000 событий / день'],
      cta: 'Выбрать Professional',
      intent: 'pro',
      accent: 'text-primary-neon',
      popular: true,
    },
    {
      name: 'Enterprise',
      level: 'Индивидуально',
      price: 'Индивидуально',
      desc: 'Для КИИ и больших контуров: безлимит, выделенные компоненты и SLA.',
      features: ['Безлимит', 'Dedicated Qdrant', 'Персональный инженер', 'SLA 99.99%'],
      cta: 'Запросить пилот',
      intent: 'enterprise',
      accent: 'text-secondary-neon',
    },
  ];

  return (
    <section className="px-6 py-40 max-w-7xl mx-auto">
      <div className="text-center mb-24">
        <h2 className="font-display text-4xl md:text-6xl font-bold uppercase tracking-tighter mb-6">
          Тарифы
        </h2>
        <p className="text-on-surface-variant font-sans opacity-70 max-w-xl mx-auto uppercase text-xs tracking-widest font-bold">
          Выберите модель внедрения под ваш контур и нагрузку
        </p>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-8">
        {tiers.map((tier, i) => (
          <motion.div
            key={tier.name}
            whileHover={{ y: -8 }}
            className={`glass-panel p-10 rounded-[2.5rem] relative flex flex-col h-full border-white/10 ${
              tier.popular ? 'bg-primary-neon/5 border-primary-neon/30' : ''
            }`}
          >
            {tier.popular && (
              <div className="absolute -top-4 left-1/2 -translate-x-1/2 bg-primary-neon text-white px-4 py-1 rounded-full text-[10px] font-display font-bold uppercase tracking-widest">
                Рекомендуется
              </div>
            )}
            <div className="mb-12">
              <span className="font-display text-[10px] tracking-[0.4em] uppercase font-bold opacity-40 mb-3 block">
                {tier.level}
              </span>
              <h3 className={`font-display text-4xl font-black uppercase ${tier.accent}`}>{tier.name}</h3>
              <div className="mt-4 font-display text-2xl md:text-3xl font-black tracking-tighter text-white">
                {tier.price}
                {tier.intent === 'pro' && <span className="text-white/40 text-sm font-sans ml-2">/ месяц</span>}
              </div>
            </div>
            <p className="text-on-surface-variant font-sans opacity-80 mb-10 text-sm leading-relaxed min-h-[4rem]">
              {tier.desc}
            </p>
            <div className="space-y-4 mb-12 flex-1">
              {tier.features.map((f) => (
                <div key={f} className="flex items-center gap-3 text-xs font-sans opacity-70">
                  <div className="w-1.5 h-1.5 rounded-full bg-primary-neon" />
                  {f}
                </div>
              ))}
            </div>
            {tier.intent === 'free' ? (
              <Link
                href="/dashboard/login"
                className={`w-full py-4 rounded-2xl font-display text-[10px] font-bold uppercase tracking-widest transition-all text-center ${
                  tier.popular ? 'bg-primary-neon text-white' : 'bg-white/5 border border-white/10 text-white hover:bg-white/10'
                }`}
              >
                {tier.cta}
              </Link>
            ) : (
              <button
                type="button"
                onClick={() => scrollToId('contact')}
                className={`w-full py-4 rounded-2xl font-display text-[10px] font-bold uppercase tracking-widest transition-all ${
                  tier.popular ? 'bg-primary-neon text-white' : 'bg-white/5 border border-white/10 text-white hover:bg-white/10'
                }`}
              >
                {tier.cta}
              </button>
            )}
          </motion.div>
        ))}
      </div>
    </section>
  );
};

const QuickScanner = () => {
  const [url, setUrl] = useState('');
  const [scanning, setScanning] = useState(false);
  const [progress, setProgress] = useState(0);
  const [result, setResult] = useState<null | { score: number; threats: string[] }>(null);

  const startScan = () => {
    if (!url) return;
    setScanning(true);
    setResult(null);
    setProgress(0);

    let p = 0;
    const interval = setInterval(() => {
      p += 2;
      setProgress(p);
      if (p >= 100) {
        clearInterval(interval);
        setScanning(false);
        setResult({
          score: 84 + Math.floor(Math.random() * 12),
          threats: ['Риск SQL-инъекции', 'Устаревший TLS', 'Публичный endpoint без rate limit'],
        });
      }
    }, 40);
  };

  return (
    <section className="px-6 py-40 max-w-7xl mx-auto">
      <div className="glass-panel rounded-[3rem] p-12 md:p-24 border border-white/10 overflow-hidden relative">
        <div className="absolute top-0 right-0 w-64 h-64 bg-primary-neon/10 blur-[100px] -mr-32 -mt-32" />

        <div className="text-center mb-16">
          <span className="text-secondary-neon font-display text-xs tracking-[0.5em] uppercase mb-4 block font-bold">
            Экспресс-аудит
          </span>
          <h2 className="font-display text-4xl md:text-5xl font-bold uppercase tracking-tighter mb-6">
            Быстрая проверка контура
          </h2>
          <p className="font-sans text-on-surface-variant opacity-70 max-w-xl mx-auto">
            Симуляция оценки экспозиции. В пилоте заменяется на интеграцию с данными заказчика.
          </p>
        </div>

        <div className="max-w-2xl mx-auto space-y-8">
          <div className="flex flex-col md:flex-row gap-4">
            <input
              type="text"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              placeholder="corp-gw.local или 10.0.0.1"
              className="flex-1 bg-white/5 border border-white/10 rounded-2xl px-6 py-4 text-on-surface focus:border-primary-neon/60 outline-none transition-all placeholder:text-white/30"
            />
            <button
              onClick={startScan}
              disabled={scanning}
              className="bg-primary-neon text-white px-10 py-4 rounded-2xl font-display text-xs font-bold uppercase tracking-widest hover:shadow-[0_0_30px_rgba(168,85,247,0.35)] transition-all disabled:opacity-50"
            >
              {scanning ? <RefreshCw className="animate-spin w-4 h-4 mx-auto" /> : 'Сканировать'}
            </button>
          </div>

          {scanning && (
            <div className="space-y-4">
              <div className="flex justify-between text-[10px] font-display uppercase tracking-widest opacity-60">
                <span>Анализ телеметрии...</span>
                <span>{progress}%</span>
              </div>
              <div className="h-1 w-full bg-white/5 rounded-full overflow-hidden">
                <motion.div initial={{ width: 0 }} animate={{ width: `${progress}%` }} className="h-full bg-primary-neon" />
              </div>
            </div>
          )}

          {result && (
            <motion.div
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              className="grid grid-cols-1 md:grid-cols-2 gap-8 p-8 rounded-3xl bg-white/5 border border-white/10"
            >
              <div className="text-center md:text-left">
                <div className="text-4xl md:text-6xl font-display font-black text-primary-neon mb-2">
                  {result.score}%
                </div>
                <div className="text-[10px] font-display uppercase tracking-widest opacity-40 font-bold">
                  Индекс экспозиции (симуляция)
                </div>
              </div>
              <div className="space-y-3">
                {result.threats.map((t) => (
                  <div key={t} className="flex items-center gap-3 text-xs text-on-surface-variant font-sans opacity-80">
                    <AlertTriangle className="w-4 h-4 text-yellow-400" />
                    {t}
                  </div>
                ))}
              </div>
            </motion.div>
          )}
        </div>
      </div>
    </section>
  );
};

const IncidentFeed = () => {
  const [incidents, setIncidents] = useState<string[]>([
    '[04:21:05] Блокировка DDoS атаки (6.4 Gbps) - RU-MOW',
    '[04:21:07] Попытка SQL-инъекции нейтрализована - US-WDC',
    '[04:21:10] Нейронная репликация узла 482 завершена',
  ]);

  useEffect(() => {
    const interval = setInterval(() => {
      const types = ['DDoS блокирован', 'Взлом предотвращен', 'Узел восстановлен', 'Брутфорс отбит'];
      const locations = ['EU-BER', 'AS-TYO', 'NA-NYC', 'RU-SPL'];
      const newIncident = `[${new Date().toLocaleTimeString()}] ${
        types[Math.floor(Math.random() * types.length)]
      } - ${locations[Math.floor(Math.random() * locations.length)]}`;
      setIncidents((prev) => [newIncident, ...prev].slice(0, 3));
    }, 4000);
    return () => clearInterval(interval);
  }, []);

  return (
    <div className="fixed bottom-10 left-10 z-[100] hidden lg:block pointer-events-none">
      <div className="space-y-3">
        {incidents.map((incident, i) => (
          <motion.div
            key={incident}
            initial={{ opacity: 0, x: -50 }}
            animate={{ opacity: 1 - i * 0.3, x: 0 }}
            className="glass-panel px-4 py-2 rounded-lg border-white/5 text-[10px] font-mono text-primary-soft/90 whitespace-nowrap shadow-xl"
          >
            {incident}
          </motion.div>
        ))}
      </div>
    </div>
  );
};

const FAQSection = () => {
  const items = [
    {
      q: 'Как быстро происходит развертывание?',
      a: 'Базовая интеграция занимает от 4 до 12 часов в зависимости от сетевой топологии. В пилоте мы ограничиваемся демонстрационным контуром и проверкой контрольных точек.',
    },
    {
      q: 'Влияет ли система на производительность?',
      a: 'В пилотном режиме основная цель — показать управляемость, контроль и воспроизводимость. Для продуктивного контура проводим отдельную оценку нагрузки и профилирование.',
    },
    {
      q: 'Как обеспечивается контроль критических действий?',
      a: 'Через связку Critic (risk/utility) + Human-in-the-Loop: high-risk шаги эскалируются, а GOD MODE требует явного подтверждения человека. Все события пишутся в Audit Trail.',
    },
    {
      q: 'Подходит ли это для air-gapped контуров?',
      a: 'Да. В Air-Gapped режиме отключаются внешние вызовы и сетевые инструменты, а LLM работает через локальный endpoint (Ollama/vLLM).',
    },
  ];

  return (
    <section id="faq" className="px-6 py-40 max-w-4xl mx-auto">
      <div className="text-center mb-20">
        <motion.span
          initial={{ opacity: 0 }}
          whileInView={{ opacity: 1 }}
          className="text-primary-neon font-display text-xs tracking-[0.5em] uppercase mb-4 block font-bold"
        >
          Информация
        </motion.span>
        <h2 className="font-display text-4xl md:text-6xl font-bold uppercase tracking-tighter mb-8">Часто задаваемые вопросы</h2>
        <div className="h-px w-20 bg-primary-neon/30 mx-auto" />
      </div>

      <div className="grid gap-6">
        {items.map((item, i) => (
          <motion.details
            key={item.q}
            className="glass-panel rounded-3xl border-white/10 group overflow-hidden"
            initial={{ opacity: 0, y: 20 }}
            whileInView={{ opacity: 1, y: 0 }}
            transition={{ delay: i * 0.06 }}
          >
            <summary className="p-8 cursor-pointer list-none flex justify-between items-center hover:bg-white/5 transition-all">
              <span className="font-display text-sm md:text-lg font-bold uppercase tracking-widest">{item.q}</span>
              <div className="w-8 h-8 rounded-full bg-white/5 flex items-center justify-center transition-transform group-open:rotate-90">
                <ChevronRight className="w-5 h-5 text-primary-neon" />
              </div>
            </summary>
            <div className="px-8 pb-8 text-on-surface-variant font-sans opacity-80 text-base leading-relaxed">
              {item.a}
            </div>
          </motion.details>
        ))}
      </div>
    </section>
  );
};

const TrustSection = () => {
  return (
    <section id="trust" className="px-6 py-40 bg-surface-dim/30">
      <div className="max-w-7xl mx-auto text-center mb-24">
        <h2 className="font-display text-4xl md:text-5xl font-bold uppercase tracking-tighter mb-8">
          Что показываем в пилоте
        </h2>
        <p className="text-on-surface-variant font-sans opacity-70 max-w-3xl mx-auto leading-relaxed">
          Спокойная демонстрация контрольной цепочки: Air-Gapped режим, управляемая эскалация (HITL), воспроизводимость аудита и прозрачность действий ReAct++.
        </p>
      </div>

      <div className="max-w-5xl mx-auto grid grid-cols-1 md:grid-cols-3 gap-6">
        {[
          { title: 'Air-Gapped', desc: 'Внешние вызовы отключены, сетевые инструменты заблокированы.', icon: Lock },
          { title: 'Human-in-the-Loop', desc: 'Критические решения требуют подтверждения человека.', icon: Headset },
          { title: 'Audit Trail', desc: 'Append-only журнал с hash-chain для доказуемости.', icon: Shield },
        ].map((c) => {
          const Icon = c.icon as any;
          return (
            <div key={c.title} className="glass-panel rounded-3xl p-10 border border-white/10">
              <div className="w-12 h-12 rounded-2xl bg-primary-neon/10 border border-primary-neon/20 flex items-center justify-center mb-8">
                <Icon className="w-6 h-6 text-primary-neon" />
              </div>
              <div className="font-display text-xl uppercase tracking-widest font-bold mb-3">{c.title}</div>
              <div className="text-on-surface-variant font-sans opacity-80 leading-relaxed text-sm">{c.desc}</div>
            </div>
          );
        })}
      </div>

      <div className="max-w-5xl mx-auto mt-20">
        <div className="flex flex-wrap justify-center items-center gap-4">
          {[
            'Large Bank',
            'Government Agency',
            'Critical Infrastructure',
            'AI Research Lab',
            'Telecom Operator',
          ].map((label) => (
            <div
              key={label}
              className="glass-panel px-6 py-3 rounded-2xl border border-white/10 text-white/70 font-mono text-[11px] tracking-widest uppercase bg-white/5 backdrop-blur-xl"
            >
              {label}
            </div>
          ))}
        </div>
      </div>
    </section>
  );
};

const ContactSection = () => {
  const api = useMemo(() => new ApiClient({ timeoutMs: 12000 }), []);
  const [loading, setLoading] = useState(false);
  const { showToast } = useToast();
  const [form, setForm] = useState({
    name: '',
    company: '',
    email: '',
    phone: '',
    message: '',
  });
  const [pdnConsent, setPdnConsent] = useState(false);

  const setField = (k: keyof typeof form) => (e: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) => {
    setForm((prev) => ({ ...prev, [k]: e.target.value }));
  };

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (loading) return;
    if (!pdnConsent) {
      showToast('Нужно согласие на обработку ПДн', 'error');
      return;
    }
    setLoading(true);
    try {
      await api.request('/api/pilot', {
        method: 'POST',
        json: {
          name: form.name.trim(),
          company: form.company.trim(),
          email: form.email.trim(),
          phone: form.phone.trim() || undefined,
          message: form.message.trim(),
        },
      });
      showToast('Заявка на пилот отправлена. Мы свяжемся с вами в ближайшее время.', 'success');
      setForm({ name: '', company: '', email: '', phone: '', message: '' });
      setPdnConsent(false);
    } catch (err: any) {
      showToast(err?.message ? `Ошибка отправки: ${String(err.message)}` : 'Ошибка отправки. Проверьте доступность API.', 'error');
    } finally {
      setLoading(false);
    }
  };

  return (
    <section id="contact" className="px-6 py-40 max-w-7xl mx-auto grid grid-cols-1 lg:grid-cols-2 gap-24 items-center">
      <div className="scroll-mt-28 outline-none">
        <motion.span
          initial={{ opacity: 0 }}
          whileInView={{ opacity: 1 }}
          className="text-primary-neon font-display text-xs tracking-[0.5em] uppercase mb-4 block font-bold"
        >
          Связаться с нами
        </motion.span>
        <h2 className="font-display text-5xl md:text-6xl font-bold uppercase tracking-tighter mb-8 max-w-md">
          Готовы к пилоту?
        </h2>
        <p className="text-on-surface-variant font-sans text-lg opacity-70 mb-16 leading-relaxed">
          Оставьте заявку: мы согласуем контур, режимы (включая air-gapped) и сценарий демонстрации для технических специалистов.
        </p>

        <div className="space-y-8">
          <div className="flex items-center gap-6 group">
            <div className="w-12 h-12 rounded-xl bg-primary-neon/10 flex items-center justify-center text-primary-neon group-hover:bg-primary-neon/20 transition-all">
              <Shield className="w-6 h-6" />
            </div>
            <span className="font-display text-[10px] tracking-widest uppercase opacity-70 font-bold">
              Контроль действий (HITL) + неизменяемый аудит
            </span>
          </div>
          <div className="flex items-center gap-6 group">
            <div className="w-12 h-12 rounded-xl bg-primary-neon/10 flex items-center justify-center text-primary-neon group-hover:bg-primary-neon/20 transition-all">
              <Zap className="w-6 h-6" />
            </div>
            <span className="font-display text-[10px] tracking-widest uppercase opacity-70 font-bold">
              Показ пилота за 10–15 минут (без “магии”, с доказуемостью)
            </span>
          </div>
        </div>
      </div>

      <motion.div
        initial={{ opacity: 0, x: 30 }}
        whileInView={{ opacity: 1, x: 0 }}
        className="glass-panel p-12 rounded-[2.5rem] relative border border-white/10"
      >
        <div className="absolute inset-0 bg-primary-neon/5 blur-3xl rounded-full" />
        <form className="relative z-10 space-y-10" onSubmit={submit}>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-8">
            <div className="space-y-2">
              <label className="text-[10px] font-display text-outline uppercase tracking-widest opacity-50 font-bold">
                Ваше имя
              </label>
              <input
                className="w-full cyber-input text-on-surface py-3"
                placeholder="Имя"
                value={form.name}
                onChange={setField('name')}
                required
              />
            </div>
            <div className="space-y-2">
              <label className="text-[10px] font-display text-outline uppercase tracking-widest opacity-50 font-bold">
                Электронная почта
              </label>
              <input
                className="w-full cyber-input text-on-surface py-3"
                placeholder="corp@domain"
                type="email"
                value={form.email}
                onChange={setField('email')}
                required
              />
            </div>
          </div>
          <div className="space-y-2">
            <label className="text-[10px] font-display text-outline uppercase tracking-widest opacity-50 font-bold">
              Компания
            </label>
            <input
              className="w-full cyber-input text-on-surface py-3"
              placeholder="Организация"
              value={form.company}
              onChange={setField('company')}
              required
            />
          </div>
          <div className="space-y-2">
            <label className="text-[10px] font-display text-outline uppercase tracking-widest opacity-50 font-bold">
              Телефон (опционально)
            </label>
            <input
              className="w-full cyber-input text-on-surface py-3"
              placeholder="+7 900 000-00-00"
              value={form.phone}
              onChange={setField('phone')}
              inputMode="tel"
            />
          </div>
          <div className="space-y-2">
            <label className="text-[10px] font-display text-outline uppercase tracking-widest opacity-50 font-bold">
              Сообщение
            </label>
            <textarea
              className="w-full cyber-input text-on-surface py-3 resize-none"
              placeholder="Контур пилота, режимы, требования"
              rows={3}
              value={form.message}
              onChange={setField('message')}
              required
            />
          </div>
          <label className="flex items-start gap-3 rounded-2xl border border-white/10 bg-white/5 px-5 py-4 text-sm text-white/70">
            <input
              type="checkbox"
              className="mt-1 accent-[#ddb7ff]"
              checked={pdnConsent}
              onChange={(e) => setPdnConsent(e.target.checked)}
              required
            />
            <span className="leading-relaxed">
              Я даю согласие на обработку персональных данных в соответствии с{' '}
              <Link href="/privacy" className="text-[#ddb7ff] hover:text-white underline underline-offset-4">
                Политикой обработки ПДн
              </Link>
              .
            </span>
          </label>
          <motion.button
            whileHover={{ scale: 1.02, boxShadow: '0 0 30px rgba(168,85,247,0.3)' }}
            whileTap={{ scale: 0.98 }}
            className="w-full bg-primary-neon text-white py-5 rounded-2xl font-display text-[11px] font-bold tracking-[0.3em] uppercase disabled:opacity-60"
            disabled={loading || !pdnConsent}
            type="submit"
          >
            {loading ? 'Отправка...' : 'Отправить запрос'}
          </motion.button>
        </form>
      </motion.div>
    </section>
  );
};

export default function Page() {
  const containerRef = useRef<HTMLDivElement>(null);
  const { scrollYProgress } = useScroll({
    target: containerRef,
    offset: ['start start', 'end end'],
  });

  const [scrolled, setScrolled] = useState(false);
  const [activeSection, setActiveSection] = useState<(typeof NAV_ITEMS)[number]['id']>('systems');

  // Premium scroll-shrinking navbar
  useEffect(() => {
    const handleScroll = () => setScrolled(window.scrollY > 40);
    window.addEventListener('scroll', handleScroll, { passive: true });
    return () => window.removeEventListener('scroll', handleScroll);
  }, []);

  useEffect(() => {
    if (typeof window === 'undefined') return;
    if (!('IntersectionObserver' in window)) return;

    const ids = NAV_ITEMS.map((x) => x.id);
    const elements = ids.map((id) => document.getElementById(id)).filter(Boolean) as HTMLElement[];
    if (!elements.length) return;

    // Pick the most "present" section (highest intersection ratio).
    const observer = new IntersectionObserver(
      (entries) => {
        let best: IntersectionObserverEntry | null = null;
        for (const e of entries) {
          if (!e.isIntersecting) continue;
          if (!best || e.intersectionRatio > best.intersectionRatio) best = e;
        }
        const id = (best?.target as HTMLElement | undefined)?.id;
        if (!id) return;
        if (ids.includes(id as any)) setActiveSection(id as any);
      },
      {
        // Treat the "middle band" as the active region so we don't jitter on edges.
        root: null,
        rootMargin: '-35% 0px -55% 0px',
        threshold: [0.05, 0.1, 0.2, 0.35, 0.5, 0.65],
      }
    );

    for (const el of elements) observer.observe(el);
    return () => observer.disconnect();
  }, []);

  const scale = useSpring(useTransform(scrollYProgress, [0, 0.2], [1, 1.04]), { stiffness: 80, damping: 32 });
  const opacity = useTransform(scrollYProgress, [0, 0.12], [1, 0]);
  const heroParallax = useTransform(scrollYProgress, [0, 0.3], [0, -40]);
  const terminalParallax = useTransform(scrollYProgress, [0.1, 0.45], [0, 30]);

  return (
    <div ref={containerRef} className="relative overflow-hidden selection:bg-primary-neon/30">
      <AnimatedSVGBackground />
      <AtmosphericOrbs />
      <BackgroundParticles />
      <IncidentFeed />

      <motion.div className="fixed top-0 left-0 right-0 h-[2px] bg-primary-neon z-[60] origin-left" style={{ scaleX: scrollYProgress }} />

      <div className="fixed inset-0 pointer-events-none z-[-1] overflow-hidden bg-[#030014]">
        <div className="absolute inset-0 bg-grid opacity-10" />
      </div>

      <nav
        className={`fixed top-0 w-full z-50 border-b border-white/[0.08] transition-all duration-300 ${
          scrolled
            ? 'bg-black/60 backdrop-blur-2xl h-16'
            : 'bg-black/20 backdrop-blur-md h-20'
        }`}
      >
        <div className="max-w-7xl mx-auto px-6 h-full flex items-center justify-between">
          <motion.button
            type="button"
            whileHover={{ opacity: 0.8 }}
            onClick={() => scrollToId('systems')}
            className="font-display text-2xl font-bold tracking-tighter text-white flex items-center gap-2 cursor-pointer"
          >
            <div className="w-6 h-6 border border-white/20 rotate-45 flex items-center justify-center">
              <div className="w-2 h-2 bg-primary-neon" />
            </div>
            AEGIS
          </motion.button>

          {/* Cinematic live status indicator */}
          <div className="hidden md:flex items-center gap-2 ml-4 text-[10px] font-mono uppercase tracking-[0.2em] text-primary-neon/70">
            <div className="w-1.5 h-1.5 rounded-full bg-primary-neon animate-pulse" />
            ALL SYSTEMS NOMINAL
          </div>

          <div className="hidden md:flex gap-12 items-center">
            {NAV_ITEMS.map((item) => {
              const isActive = activeSection === item.id;
              return (
              <button
                key={item.label}
                type="button"
                onClick={() => scrollToId(item.id)}
                aria-current={isActive ? 'page' : undefined}
                className={[
                  'font-display text-[10px] lowercase tracking-[0.4em] transition-all font-bold relative',
                  isActive ? 'text-white nav-link-active' : 'text-white/40 hover:text-white',
                ].join(' ')}
              >
                {item.label}
              </button>
              );
            })}
          </div>
          <div className="flex items-center gap-3">
            <button
              type="button"
              onClick={() => scrollToId('threat-map')}
              className="hidden md:inline-flex border border-white/10 text-white/70 px-6 py-2.5 rounded-full font-display text-[10px] tracking-widest font-bold uppercase hover:bg-white/5 transition-all"
            >
              Смотреть демо
            </button>
            <Link
              href="/dashboard/overview"
              className="bg-white text-black px-8 py-2.5 rounded-full font-display text-[10px] tracking-widest font-bold uppercase transition-all hover:shadow-[0_0_30px_rgba(168,85,247,0.25)] noir-shimmer"
            >
              Запустить пилот
            </Link>
          </div>
        </div>
      </nav>

      <section id="systems" className="relative px-6 pt-48 pb-32 max-w-7xl mx-auto min-h-screen flex flex-col items-center">
        <motion.div initial={{ opacity: 0, y: 30 }} animate={{ opacity: 1, y: 0 }} style={{ opacity, y: heroParallax }} className="text-center mb-24 z-10">
          <p className="text-primary-soft font-display text-xs md:text-sm uppercase tracking-[0.35em] mb-6 font-bold">
            Готово к пилотным проектам.
          </p>
          <h1 className="font-display text-4xl sm:text-5xl md:text-6xl font-bold tracking-tighter text-white max-w-5xl mx-auto leading-[1.08]">
            Автономная иммунная система
            <br />
            для цифровой инфраструктуры
          </h1>
          <p className="mt-6 text-xl md:text-2xl text-white/70 max-w-3xl mx-auto font-sans leading-relaxed">
            Zero-Trust на 100%.
            <br />
            Self-Healing • Federation • Moving Target Defense • Advanced Deception
          </p>
          <div className="mt-10 flex flex-wrap justify-center gap-4">
            <Link
              href="#pilot"
              className="inline-flex items-center justify-center bg-primary-neon text-white px-10 py-4 rounded-full font-display text-[11px] tracking-[0.28em] font-bold uppercase glow-hover hover:shadow-[0_0_30px_rgba(168,85,247,0.35)] transition-all"
            >
              Записаться на пилот
            </Link>
            <Link
              href="/dashboard"
              className="inline-flex items-center justify-center border border-white/15 text-white/90 px-10 py-4 rounded-full font-display text-[11px] tracking-[0.28em] font-bold uppercase bg-white/5 hover:bg-white/10 transition-all"
            >
              Смотреть демо
            </Link>
          </div>
        </motion.div>

        <motion.div initial={{ opacity: 0 }} whileInView={{ opacity: 1 }} className="w-full grid grid-cols-2 md:grid-cols-4 gap-12 border-t border-white/5 pt-16 mb-40">
          {[
            { label: 'Задержка', value: '< 100 мс' },
            { label: 'Точность', value: '99.98%' },
            { label: 'Узлы', value: '42K+' },
            { label: 'Аптайм', value: '100%' },
          ].map((stat, i) => (
            <motion.div key={i} initial={{ opacity: 0, y: 20 }} whileInView={{ opacity: 1, y: 0 }} transition={{ delay: i * 0.1 }} className="text-center">
              <div className="text-primary-neon font-display text-4xl mb-2 font-bold tracking-tighter">{stat.value}</div>
              <div className="text-outline font-display text-[10px] uppercase tracking-[0.3em] font-bold opacity-60">{stat.label}</div>
            </motion.div>
          ))}
        </motion.div>

        <motion.div style={{ scale, y: terminalParallax }} className="relative w-full aspect-video md:aspect-[21/9] z-0 overflow-hidden rounded-[2rem] border border-white/5 glass-panel">
          <div className="absolute inset-0 bg-gradient-to-t from-background-base/80 to-transparent z-10" />
          <div className="absolute inset-0 flex items-center justify-center">
            <Shield className="w-1/2 h-1/2 text-primary-neon/10 animate-pulse" strokeWidth={0.5} />
          </div>
          <Terminal />
        </motion.div>
      </section>

      <WhatWeBuiltSection />

      <KeyCapabilitiesSection />

      <PilotSection />

      <ThreatMap />
      <div className="section-transition" />
      <ThreatTicker />
      <div className="section-transition" />
      <QuickScanner />
      <div className="section-transition" />
      <DefenseTiers />
      <div className="section-transition" />
      <FAQSection />
      <div className="section-transition" />
      <TrustSection />
      <div className="section-transition" />
      <ContactSection />

      <footer className="w-full py-20 border-t border-white/5 bg-surface-dim/50 backdrop-blur-3xl">
        <div className="max-w-7xl mx-auto px-6 flex flex-col md:flex-row justify-between items-center gap-12">
          <div className="font-display text-2xl font-black text-white/20 hover:text-white/40 transition-colors cursor-pointer">AEGIS</div>
          <div className="flex flex-col md:flex-row gap-6 md:gap-12 items-center">
            <Link
              href="/privacy"
              className="font-display text-[10px] uppercase tracking-widest text-outline/60 hover:text-white transition-colors"
            >
              Политика обработки персональных данных
            </Link>
            <a
              href="mailto:privacy@aegis-security.ru"
              className="font-display text-[10px] uppercase tracking-widest text-outline/60 hover:text-white transition-colors"
            >
              Оператор персональных данных: Максим Очередько
            </a>
          </div>
          <div className="text-[10px] font-display uppercase tracking-widest text-outline/20 font-bold">© 2026 AEGIS DIGITAL IMMUNITY.</div>
        </div>
      </footer>
    </div>
  );
}
