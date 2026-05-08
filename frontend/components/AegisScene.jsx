'use client';
import { Canvas, useFrame } from '@react-three/fiber';
import { Points, PointMaterial } from '@react-three/drei';
import * as THREE from 'three';
import { useRef, useMemo } from 'react';
import { EffectComposer, Bloom, ChromaticAberration } from '@react-three/postprocessing';
import CrystalShield from './CrystalShield';
import ShatterSystem, { Shockwave, CoreFlash } from './ShatterSystem';

function DNAStrand({ color, offset = 0, nodes = 1000, convergence = 0, speed = 1, bloomIntensity = 0 }) {
  const ref = useRef();
  const targetRadius = useRef(2.0);
  const matRef = useRef();

  const positions = useMemo(() => {
    const pos = new Float32Array(nodes * 3);
    for (let i = 0; i < nodes; i++) {
      const angle = (i / nodes) * Math.PI * 10 + offset;
      pos.set([Math.cos(angle) * 2.0, (i / nodes) * 10 - 5, Math.sin(angle) * 2.0], i * 3);
    }
    return pos;
  }, [nodes, offset]);

  useFrame((state) => {
    const t = state.clock.getElapsedTime();
    if (ref.current) ref.current.rotation.y = t * 0.3 * speed;

    const tr = 2.0 - convergence * 1.5;
    targetRadius.current = THREE.MathUtils.lerp(targetRadius.current, tr, 0.05);

    const arr = ref.current?.geometry?.attributes?.position?.array;
    if (arr) {
      for (let i = 0; i < nodes; i++) {
        const angle = (i / nodes) * Math.PI * 10 + offset;
        arr[i * 3] = Math.cos(angle) * targetRadius.current;
        arr[i * 3 + 2] = Math.sin(angle) * targetRadius.current;
      }
      ref.current.geometry.attributes.position.needsUpdate = true;
    }

    if (matRef.current) {
      matRef.current.size = 0.06 + convergence * 0.08 + bloomIntensity * 0.04;
    }
  });

  return (
    <Points ref={ref} positions={positions} stride={3}>
      <PointMaterial
        ref={matRef}
        transparent
        color={color}
        size={0.06}
        sizeAttenuation
        depthWrite={false}
        blending={THREE.AdditiveBlending}
      />
    </Points>
  );
}

export default function AegisScene({ config = {}, scrollProgress = 0, bloomTrigger = 0 }) {
  const nodes = config.nodes || 1000;
  const speed = config.agents === 'Unlimited' ? 3 : config.agents || 1;
  const c1 = config.color || '#00ffff';
  const c2 = config.color === '#ffffff' ? '#ff88ff' : '#ff00ff';

  const bloomBase = scrollProgress > 0.8 ? (scrollProgress - 0.8) * 5 : 0;
  const bloomIntensity = bloomBase + (bloomTrigger > 0 ? 2.0 : 0);

  // Конвертация scrollProgress в фазы ShatterSystem
  // 0.0 - 0.5: ДНК-спирали видны, осколки вибрируют
  // 0.5 - 0.7: разлёт (shatter)
  // 0.7 - 1.0: сборка в щит
  const shatterProgress = (scrollProgress - 0.4) * 2; // смещаем диапазон

  return (
    <div className="fixed inset-0 -z-10">
      <Canvas camera={{ position: [0, 0, 10], fov: 60 }}>
        <color attach="background" args={['#000000']} />
        <ambientLight intensity={0.2} />

        {/* ДНК-спирали (исчезают при трансмутации) */}
        <group visible={scrollProgress < 0.65}>
          <DNAStrand
            color={c1}
            offset={0}
            nodes={nodes}
            convergence={scrollProgress}
            speed={speed}
            bloomIntensity={bloomIntensity}
          />
          <DNAStrand
            color={c2}
            offset={Math.PI}
            nodes={nodes}
            convergence={scrollProgress}
            speed={speed}
            bloomIntensity={bloomIntensity}
          />
        </group>

        {/* Shatter-система: осколки трансмутации */}
        <ShatterSystem
          visible={scrollProgress > 0.3}
          progress={Math.max(0, Math.min(1, shatterProgress))}
          color1={c1}
          color2={c2}
          particleCount={2000}
          bloomIntensity={bloomIntensity}
        />

        {/* Ударная волна при разлёте */}
        <Shockwave
          trigger={scrollProgress > 0.5 ? 1 : 0}
          position={[0, 0, 0]}
        />

        {/* Вспышка в ядре */}
        <CoreFlash
          trigger={scrollProgress > 0.65 ? 1 : 0}
          position={[0, 0, 0]}
        />

        {/* Кристаллический щит (появляется после сборки) */}
        <CrystalShield
          visible={scrollProgress > 0.85}
          bloomIntensity={bloomIntensity}
        />

        {/* Постобработка */}
        <EffectComposer>
          <Bloom
            luminanceThreshold={0.3}
            luminanceSmoothing={0.9}
            intensity={0.5 + bloomIntensity * 0.5}
          />
          <ChromaticAberration
            offset={[0.001 + bloomIntensity * 0.002, 0.001 + bloomIntensity * 0.002]}
          />
        </EffectComposer>
      </Canvas>
    </div>
  );
}