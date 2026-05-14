'use client';

import { useEffect } from 'react';
import { useRouter } from 'next/navigation';

export default function DashboardRedirect() {
  const router = useRouter();
  
  useEffect(() => {
    router.replace('/dashboard/overview');
  }, [router]);

  return <div className="flex h-screen items-center justify-center text-white/40">Redirecting to War Room...</div>;
}
