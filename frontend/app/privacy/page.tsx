'use client';

import React from 'react';
import Link from 'next/link';
import { motion } from 'framer-motion';
import { Shield, Lock } from 'lucide-react';

export default function PrivacyPage() {
  return (
    <div className="min-h-screen bg-[#030014] text-white relative overflow-hidden">
      <div className="fixed inset-0 pointer-events-none z-[-1] overflow-hidden bg-[#030014]">
        <div className="absolute inset-0 bg-grid opacity-10" />
        <div className="absolute inset-0 bg-[radial-gradient(circle_at_50%_50%,rgba(168,85,247,0.08),rgba(3,0,20,1))]" />
      </div>

      <header className="sticky top-0 z-50 bg-black/20 backdrop-blur-md border-b border-white/[0.08]">
        <div className="max-w-5xl mx-auto px-6 h-20 flex items-center justify-between">
          <Link href="/" className="flex items-center gap-3">
            <div className="w-10 h-10 rounded-xl bg-white/5 border border-white/10 flex items-center justify-center">
              <Shield className="w-5 h-5 text-[#ddb7ff]" />
            </div>
            <div>
              <div className="font-display font-black tracking-tighter text-lg">AEGIS</div>
              <div className="text-[10px] font-mono tracking-[0.25em] uppercase text-white/40">privacy</div>
            </div>
          </Link>
          <Link
            href="/#contact"
            className="px-5 py-2 rounded-2xl bg-white/5 border border-white/10 text-xs font-mono tracking-widest hover:bg-white/10"
          >
            Связаться
          </Link>
        </div>
      </header>

      <main className="max-w-5xl mx-auto px-6 py-20">
        <motion.div initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }} className="glass-panel rounded-[2.5rem] border border-white/10 p-10 md:p-14">
          <div className="flex items-start justify-between gap-8 flex-col md:flex-row">
            <div>
              <div className="text-[#ddb7ff] font-display text-xs tracking-[0.5em] uppercase font-bold">
                152‑ФЗ / Роскомнадзор
              </div>
              <h1 className="mt-4 font-display text-3xl md:text-5xl font-black tracking-tighter uppercase">
                Политика обработки персональных данных
              </h1>
              <div className="mt-4 text-white/60 font-mono text-xs tracking-widest">
                Актуально на: май 2026 • Версия: 1.0 • Дата публикации: {new Date().toLocaleDateString('ru-RU')}
              </div>
            </div>
            <div className="flex items-center gap-3 text-xs font-mono tracking-widest text-white/60">
              <Lock className="w-4 h-4 text-[#00F5A3]" />
              ZERO‑TRUST
            </div>
          </div>

          <div className="mt-10 space-y-10 text-white/70 leading-relaxed">
            <section>
              <h2 className="font-display text-lg md:text-xl font-bold tracking-widest uppercase text-white mb-3">
                1. Общие положения
              </h2>
              <p>
                Настоящая Политика определяет порядок и условия обработки персональных данных посетителей сайта{' '}
                <span className="text-white">aegis-security.ru</span> (далее — «Сайт») оператором персональных данных.
                Политика составлена с учётом требований Федерального закона РФ №152‑ФЗ «О персональных данных», а также
                рекомендаций уполномоченного органа по защите прав субъектов персональных данных.
              </p>
            </section>

            <section className="grid grid-cols-1 md:grid-cols-2 gap-6">
              <div className="rounded-3xl border border-white/10 bg-white/5 p-6">
                <div className="text-[10px] font-mono tracking-[0.25em] uppercase text-white/40 mb-2">Оператор</div>
                <div className="font-display text-xl font-bold text-white">Максим Очередько</div>
                <div className="mt-2 text-sm text-white/70">
                  Контакты для обращений субъектов ПДн:{' '}
                  <a href="mailto:privacy@aegis-security.ru" className="text-[#ddb7ff] hover:text-white underline underline-offset-4">
                    privacy@aegis-security.ru
                  </a>
                </div>
              </div>
              <div className="rounded-3xl border border-white/10 bg-white/5 p-6">
                <div className="text-[10px] font-mono tracking-[0.25em] uppercase text-white/40 mb-2">Назначение</div>
                <div className="text-sm">
                  Обработка заявок на пилот, обратная связь, обеспечение работоспособности Сайта и защита от злоупотреблений
                  (например, спама и атак).
                </div>
              </div>
            </section>

            <section>
              <h2 className="font-display text-lg md:text-xl font-bold tracking-widest uppercase text-white mb-3">
                2. Какие данные мы обрабатываем
              </h2>
              <ul className="list-disc pl-6 space-y-2">
                <li>
                  <span className="text-white">Данные, предоставляемые в форме заявки</span>: имя, компания, email, телефон
                  (опционально), сообщение.
                </li>
                <li>
                  <span className="text-white">Технические данные</span>: IP-адрес, сведения о браузере/устройстве, дата и время
                  запроса, cookies (если включены).
                </li>
              </ul>
            </section>

            <section>
              <h2 className="font-display text-lg md:text-xl font-bold tracking-widest uppercase text-white mb-3">
                3. Цели и правовые основания обработки
              </h2>
              <div className="space-y-3">
                <p>
                  Мы обрабатываем персональные данные на основании{' '}
                  <span className="text-white">согласия субъекта персональных данных</span> (ст. 6, ст. 9 152‑ФЗ),
                  предоставляемого путём отметки чекбокса согласия в форме заявки и отправки формы.
                </p>
                <p>
                  Цели: рассмотрение заявки на пилот, связь с заявителем, подготовка демонстрации/пилотного контура, ведение
                  переписки по запросу.
                </p>
              </div>
            </section>

            <section>
              <h2 className="font-display text-lg md:text-xl font-bold tracking-widest uppercase text-white mb-3">
                4. Порядок и условия обработки
              </h2>
              <ul className="list-disc pl-6 space-y-2">
                <li>Обработка включает сбор, запись, систематизацию, хранение, уточнение, использование и удаление.</li>
                <li>Данные обрабатываются с применением автоматизированных и неавтоматизированных средств.</li>
                <li>Доступ к данным ограничивается принципом минимальных привилегий.</li>
              </ul>
            </section>

            <section>
              <h2 className="font-display text-lg md:text-xl font-bold tracking-widest uppercase text-white mb-3">
                5. Cookies и аналитика
              </h2>
              <p>
                Сайт может использовать cookies. Необходимые cookies обеспечивают базовую работоспособность. Дополнительные
                категории (аналитические/маркетинговые) включаются только при вашем выборе в баннере cookies.
              </p>
              <p className="mt-2">
                Вы можете изменить выбор cookies, очистив сохранённое решение в настройках браузера (локальное хранилище) и
                повторно выбрав параметры в баннере.
              </p>
            </section>

            <section>
              <h2 className="font-display text-lg md:text-xl font-bold tracking-widest uppercase text-white mb-3">
                6. Передача третьим лицам
              </h2>
              <p>
                Мы не продаём персональные данные. В рамках предоставления сервиса и обработки заявок данные могут передаваться
                третьим лицам в объёме, необходимом для достижения целей обработки и исполнения обязательств.
              </p>
              <div className="mt-3 rounded-3xl border border-white/10 bg-white/5 p-6">
                <div className="text-[10px] font-mono tracking-[0.25em] uppercase text-white/40 mb-2">
                  Возможные получатели / подрядчики
                </div>
                <ul className="list-disc pl-6 space-y-2 text-sm">
                  <li>
                    <span className="text-white">Vercel Inc.</span> — хостинг и доставка статического контента (CDN).
                  </li>
                  <li>
                    <span className="text-white">xAI / OpenRouter</span> — обработка запросов через Grok (если включён соответствующий режим и
                    требуется обработка).
                  </li>
                  <li>
                    <span className="text-white">Иные подрядчики по договору</span> — при необходимости интеграций, поддержки или сопровождения
                    (в пределах согласованных целей).
                  </li>
                </ul>
              </div>
            </section>

            <section>
              <h2 className="font-display text-lg md:text-xl font-bold tracking-widest uppercase text-white mb-3">
                7. Сроки хранения
              </h2>
              <p>
                Персональные данные хранятся не дольше, чем это требуется целями обработки, либо до отзыва согласия субъектом
                персональных данных, если иное не требуется законом.
              </p>
            </section>

            <section>
              <h2 className="font-display text-lg md:text-xl font-bold tracking-widest uppercase text-white mb-3">
                8. Права субъекта и отзыв согласия
              </h2>
              <p>
                Вы вправе запросить сведения об обработке ваших данных, потребовать уточнения/удаления, а также отозвать
                согласие. Для этого направьте обращение на{' '}
                <a href="mailto:privacy@aegis-security.ru" className="text-[#ddb7ff] hover:text-white underline underline-offset-4">
                  privacy@aegis-security.ru
                </a>{' '}
                с темой «Отзыв согласия / персональные данные».
              </p>
            </section>

            <section>
              <h2 className="font-display text-lg md:text-xl font-bold tracking-widest uppercase text-white mb-3">
                9. Меры защиты
              </h2>
              <ul className="list-disc pl-6 space-y-2">
                <li>Шифрование соединений (HTTPS).</li>
                <li>Логирование действий и контроль доступа.</li>
                <li>Минимизация данных и сроков хранения.</li>
              </ul>
            </section>

            <section>
              <h2 className="font-display text-lg md:text-xl font-bold tracking-widest uppercase text-white mb-3">
                10. Обновление политики
              </h2>
              <p>
                Мы можем обновлять Политику. Актуальная версия всегда доступна на этой странице. Существенные изменения
                публикуются с обновлением версии и даты.
              </p>
            </section>
          </div>
        </motion.div>

        <div className="mt-8 text-center text-xs text-white/40 font-mono tracking-widest">
          <Link href="/" className="hover:text-white underline underline-offset-4">
            Вернуться на сайт
          </Link>
        </div>
      </main>
    </div>
  );
}

