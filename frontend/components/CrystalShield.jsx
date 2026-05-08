'use client';
import { useRef, useMemo } from 'react';
import { useFrame } from '@react-three/fiber';
import { Points, PointMaterial } from '@react-three/drei';
import * as THREE from 'three';

// Внутренняя стабильная ДНК-спираль внутри щита
function InnerHelix({ color1 = '#0088ff', color2 = '#aa00ff', visible = 0 }) {
  const ref1 = useRef();
  const ref2 = useRef();
  const nodes = 400;

  const positions1 = useMemo(() => {
    const pos = new Float32Array(nodes * 3);
    for (let i = 0; i < nodes; i++) {
      const angle = (i / nodes) * Math.PI * 8;
      const r = 0.6;
      const y = (i / nodes) * 3 - 1.5;
      pos.set([Math.cos(angle) * r, y, Math.sin(angle) * r], i * 3);
    }
    return pos;
  }, []);

  const positions2 = useMemo(() => {
    const pos = new Float32Array(nodes * 3);
    for (let i = 0; i < nodes; i++) {
      const angle = (i / nodes) * Math.PI * 8 + Math.PI;
      const r = 0.6;
      const y = (i / nodes) * 3 - 1.5;
      pos.set([Math.cos(angle) * r, y, Math.sin(angle) * r], i * 3);
    }
    return pos;
  }, []);

  useFrame((state) => {
    const t = state.clock.getElapsedTime();
    if (ref1.current) ref1.current.rotation.y = t * 0.2;
    if (ref2.current) ref2.current.rotation.y = t * 0.2;
  });

  return (
    <group visible={visible > 0.3}>
      <Points ref={ref1} positions={positions1} stride={3}>
        <PointMaterial
          transparent
          color={color1}
          size={0.03}
          sizeAttenuation
          depthWrite={false}
          blending={THREE.AdditiveBlending}
          opacity={Math.min(visible * 2, 1)}
        />
      </Points>
      <Points ref={ref2} positions={positions2} stride={3}>
        <PointMaterial
          transparent
          color={color2}
          size={0.03}
          sizeAttenuation
          depthWrite={false}
          blending={THREE.AdditiveBlending}
          opacity={Math.min(visible * 2, 1)}
        />
      </Points>
    </group>
  );
}

// Осколки, формирующие внешнюю оболочку щита
function ShieldFragments({ visible = 0 }) {
  const ref = useRef();
  const count = 300;

  const positions = useMemo(() => {
    const pos = new Float32Array(count * 3);
    // Икосаэдрическое распределение точек на сфере
    const phi = Math.PI * (3 - Math.sqrt(5));
    for (let i = 0; i < count; i++) {
      const y = 1 - (i / (count - 1)) * 2;
      const radius = Math.sqrt(1 - y * y);
      const theta = phi * i;
      pos.set([
        Math.cos(theta) * radius * 2.5,
        y * 2.5,
        Math.sin(theta) * radius * 2.5,
      ], i * 3);
    }
    return pos;
  }, []);

  const sizes = useMemo(() => {
    const arr = new Float32Array(count);
    for (let i = 0; i < count; i++) {
      arr[i] = Math.random() * 0.15 + 0.04;
    }
    return arr;
  }, []);

  useFrame((state) => {
    if (ref.current) {
      ref.current.rotation.y = state.clock.getElapsedTime() * 0.1;
      ref.current.rotation.x = Math.sin(state.clock.getElapsedTime() * 0.05) * 0.1;
    }
  });

  return (
    <Points ref={ref} positions={positions} stride={3}>
      <PointMaterial
        transparent
        color="#00ffff"
        size={0.08}
        sizeAttenuation
        depthWrite={false}
        blending={THREE.AdditiveBlending}
        opacity={Math.min(visible * 3, 1)}
      />
    </Points>
  );
}

// Основная стеклянная геометрия щита
function GlassShield({ visible = 0, bloomIntensity = 0 }) {
  const meshRef = useRef();
  const materialRef = useRef();

  useFrame((state) => {
    if (materialRef.current) {
      // Пульсация прозрачности
      const pulse = Math.sin(state.clock.getElapsedTime() * 2) * 0.05 + 0.35;
      materialRef.current.opacity = THREE.MathUtils.lerp(
        materialRef.current.opacity,
        visible > 0 ? pulse : 0,
        0.05
      );
      // Пульсация эмиссии
      materialRef.current.emissiveIntensity = 0.3 + bloomIntensity * 0.5 + Math.sin(state.clock.getElapsedTime() * 3) * 0.1;
    }
    if (meshRef.current) {
      meshRef.current.rotation.y = state.clock.getElapsedTime() * 0.08;
      meshRef.current.rotation.x = Math.sin(state.clock.getElapsedTime() * 0.04) * 0.05;
    }
  });

  return (
    <mesh ref={meshRef} visible={visible > 0.1}>
      <icosahedronGeometry args={[2.5, 4]} />
      <meshPhysicalMaterial
        ref={materialRef}
        color="#00ffff"
        emissive="#0066aa"
        emissiveIntensity={0.3}
        metalness={0.05}
        roughness={0.1}
        transmission={0.85}
        thickness={0.6}
        ior={1.5}
        transparent
        opacity={0}
        side={THREE.DoubleSide}
        depthWrite={false}
      />
    </mesh>
  );
}

// Силовое поле — дополнительный wireframe-слой
function Forcefield({ visible = 0, bloomIntensity = 0 }) {
  const ref = useRef();

  useFrame((state) => {
    if (ref.current) {
      ref.current.rotation.y = state.clock.getElapsedTime() * 0.15;
      ref.current.rotation.z = state.clock.getElapsedTime() * 0.1;
      ref.current.material.opacity = (0.15 + Math.sin(state.clock.getElapsedTime() * 4) * 0.05) * Math.min(visible * 2, 1);
    }
  });

  return (
    <mesh ref={ref} visible={visible > 0.2}>
      <icosahedronGeometry args={[2.7, 2]} />
      <meshBasicMaterial
        color="#00ffff"
        wireframe
        transparent
        opacity={0}
        depthWrite={false}
        blending={THREE.AdditiveBlending}
      />
    </mesh>
  );
}

// Главный экспортируемый компонент
export default function CrystalShield({ visible = false, bloomIntensity = 0 }) {
  const targetVisible = visible ? 1 : 0;
  const smoothVisible = useRef(0);

  useFrame(() => {
    smoothVisible.current = THREE.MathUtils.lerp(smoothVisible.current, targetVisible, 0.04);
  });

  return (
    <group>
      <GlassShield visible={smoothVisible.current} bloomIntensity={bloomIntensity} />
      <Forcefield visible={smoothVisible.current} bloomIntensity={bloomIntensity} />
      <ShieldFragments visible={smoothVisible.current} />
      <InnerHelix
        color1="#0088ff"
        color2="#cc44ff"
        visible={smoothVisible.current}
      />
    </group>
  );
}