'use client';

import { useEffect } from 'react';
import { useRouter } from 'next/navigation';

/** Старые ссылки /dashboard/login → кабинет с формой входа. */
export default function DashboardLoginRedirect() {
  const router = useRouter();

  useEffect(() => {
    router.replace('/dashboard/overview');
  }, [router]);

  return (
    <div className="flex h-screen items-center justify-center text-white/50 font-mono text-sm">
      Переход в личный кабинет…
    </div>
  );
}
