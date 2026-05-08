'use client';
import { useRef, useMemo, useEffect } from 'react';
import { useFrame } from '@react-three/fiber';
import { Points, PointMaterial } from '@react-three/drei';
import * as THREE from 'three';

// Утилита: генерация случайной точки на поверхности сферы
function randomSpherePoint(radius = 1) {
  const u = Math.random();
  const v = Math.random();
  const theta = 2 * Math.PI * u;
  const phi = Math.acos(2 * v - 1);
  return new THREE.Vector3(
    radius * Math.sin(phi) * Math.cos(theta),
    radius * Math.sin(phi) * Math.sin(theta),
    radius * Math.cos(phi)
  );
}

// Утилита: генерация точки на спирали (как в ДНК)
function spiralPoint(t, radius = 2.0, height = 8, turns = 10) {
  const angle = t * Math.PI * 2 * turns;
  const y = (t - 0.5) * height;
  return new THREE.Vector3(
    Math.cos(angle) * radius,
    y,
    Math.sin(angle) * radius
  );
}

export default function ShatterSystem({
  visible = false,
  progress = 0,        // 0 = ДНК, 0.5 = разлёт, 1 = собраны в щит
  color1 = '#00aaff',
  color2 = '#cc44ff',
  particleCount = 2000,
  bloomIntensity = 0,
}) {
  const pointsRef = useRef();
  const materialRef = useRef();

  // Храним исходные позиции (ДНК) и целевые позиции (щит)
  const origins = useRef(new Float32Array(particleCount * 3));
  const targets = useRef(new Float32Array(particleCount * 3));
  const velocities = useRef(new Float32Array(particleCount * 3));
  const colors = useRef(new Float32Array(particleCount * 3));

  // Инициализация один раз
  useEffect(() => {
    const orig = origins.current;
    const targ = targets.current;
    const vel = velocities.current;
    const col = colors.current;

    const color1Obj = new THREE.Color(color1);
    const color2Obj = new THREE.Color(color2);

    for (let i = 0; i < particleCount; i++) {
      // Исходная позиция — ДНК-спираль
      const t = (i / particleCount) * 2; // 2 полных цикла
      const strand = i % 2 === 0 ? 0 : Math.PI; // чередуем две нити
      const spiralPos = spiralPoint(
        ((i % (particleCount / 2)) / (particleCount / 2)),
        2.0,
        8,
        10
      );

      // Добавляем смещение для второй нити
      if (strand === Math.PI) {
        const angle = (i / particleCount) * Math.PI * 2 * 10 + Math.PI;
        orig[i * 3] = Math.cos(angle) * 2.0;
        orig[i * 3 + 1] = ((i % (particleCount / 2)) / (particleCount / 2)) * 8 - 4;
        orig[i * 3 + 2] = Math.sin(angle) * 2.0;
      } else {
        const angle = (i / particleCount) * Math.PI * 2 * 10;
        orig[i * 3] = Math.cos(angle) * 2.0;
        orig[i * 3 + 1] = ((i % (particleCount / 2)) / (particleCount / 2)) * 8 - 4;
        orig[i * 3 + 2] = Math.sin(angle) * 2.0;
      }

      // Целевая позиция — икосаэдрическая сфера (щит)
      const spherePoint = randomSpherePoint(2.5);
      targ[i * 3] = spherePoint.x;
      targ[i * 3 + 1] = spherePoint.y;
      targ[i * 3 + 2] = spherePoint.z;

      // Случайная скорость для разлёта
      const scatterDir = randomSpherePoint(1);
      const speed = Math.random() * 5 + 2;
      vel[i * 3] = scatterDir.x * speed;
      vel[i * 3 + 1] = scatterDir.y * speed;
      vel[i * 3 + 2] = scatterDir.z * speed;

      // Цвет — смесь между синим и фиолетовым
      const mixed = color1Obj.clone().lerp(color2Obj, Math.random());
      col[i * 3] = mixed.r;
      col[i * 3 + 1] = mixed.g;
      col[i * 3 + 2] = mixed.b;
    }
  }, [particleCount, color1, color2]);

  // Обновление позиций каждый кадр
  useFrame((state) => {
    if (!pointsRef.current) return;
    if (!visible && progress < 0.01) {
      pointsRef.current.visible = false;
      return;
    }

    pointsRef.current.visible = true;
    const arr = pointsRef.current.geometry.attributes.position.array;
    const orig = origins.current;
    const targ = targets.current;
    const vel = velocities.current;

    const t = state.clock.getElapsedTime();

    for (let i = 0; i < particleCount; i++) {
      const i3 = i * 3;

      if (progress < 0.3) {
        // Фаза 1: частицы на ДНК-спирали, начинают вибрировать
        const vibration = (1 - progress / 0.3) * 0.05;
        arr[i3] = orig[i3] + Math.sin(t * 10 + i) * vibration;
        arr[i3 + 1] = orig[i3 + 1] + Math.cos(t * 10 + i) * vibration;
        arr[i3 + 2] = orig[i3 + 2] + Math.sin(t * 10 + i + 2) * vibration;
      } else if (progress < 0.65) {
        // Фаза 2: разлёт (shatter)
        const phase2Progress = (progress - 0.3) / 0.35; // 0..1
        const scatterAmount = easeOutExpo(phase2Progress) * 6;
        arr[i3] = orig[i3] + vel[i3] * scatterAmount * (1 + Math.sin(t * 3 + i) * 0.3);
        arr[i3 + 1] = orig[i3 + 1] + vel[i3 + 1] * scatterAmount * (1 + Math.cos(t * 3 + i) * 0.3);
        arr[i3 + 2] = orig[i3 + 2] + vel[i3 + 2] * scatterAmount * (1 + Math.sin(t * 3 + i + 1) * 0.3);
      } else {
        // Фаза 3: сборка в щит
        const phase3Progress = (progress - 0.65) / 0.35; // 0..1
        const eased = easeInOutCubic(phase3Progress);
        const wobble = (1 - phase3Progress) * 0.15 * Math.sin(t * 5 + i);

        arr[i3] = THREE.MathUtils.lerp(
          orig[i3] + vel[i3] * 6,
          targ[i3],
          eased
        ) + wobble;
        arr[i3 + 1] = THREE.MathUtils.lerp(
          orig[i3 + 1] + vel[i3 + 1] * 6,
          targ[i3 + 1],
          eased
        ) + wobble;
        arr[i3 + 2] = THREE.MathUtils.lerp(
          orig[i3 + 2] + vel[i3 + 2] * 6,
          targ[i3 + 2],
          eased
        ) + wobble;
      }
    }

    pointsRef.current.geometry.attributes.position.needsUpdate = true;

    // Размер частиц меняется от фазы
    if (materialRef.current) {
      if (progress < 0.3) {
        materialRef.current.size = 0.04;
      } else if (progress < 0.65) {
        materialRef.current.size = 0.04 + (progress - 0.3) * 0.12;
      } else {
        materialRef.current.size = 0.08 - (progress - 0.65) * 0.04;
      }
    }
  });

  // Функции плавности
  function easeOutExpo(x) {
    return x === 1 ? 1 : 1 - Math.pow(2, -10 * x);
  }
  function easeInOutCubic(x) {
    return x < 0.5 ? 4 * x * x * x : 1 - Math.pow(-2 * x + 2, 3) / 2;
  }

  const positions = useMemo(() => {
    const pos = new Float32Array(particleCount * 3);
    pos.set(origins.current);
    return pos;
  }, [particleCount]);

  return (
    <Points ref={pointsRef} positions={positions} stride={3} frustumCulled={false}>
      <PointMaterial
        ref={materialRef}
        transparent
        vertexColors
        size={0.04}
        sizeAttenuation
        depthWrite={false}
        blending={THREE.AdditiveBlending}
      />
    </Points>
  );
}

// Дополнительный компонент: ударная волна при трансмутации
export function Shockwave({ trigger = 0, position = [0, 0, 0] }) {
  const ringRef = useRef();
  const materialRef = useRef();
  const startTime = useRef(0);

  useEffect(() => {
    startTime.current = performance.now() / 1000;
  }, [trigger]);

  useFrame((state) => {
    if (!ringRef.current || !materialRef.current) return;
    const elapsed = state.clock.getElapsedTime() - startTime.current;
    const duration = 2.0;

    if (elapsed < duration) {
      const progress = elapsed / duration;
      const scale = 0.1 + progress * 8;
      ringRef.current.scale.setScalar(scale);
      materialRef.current.opacity = (1 - progress) * 0.6;
    } else {
      materialRef.current.opacity = 0;
    }
  });

  return (
    <mesh ref={ringRef} position={position}>
      <ringGeometry args={[0.8, 1.2, 64]} />
      <meshBasicMaterial
        ref={materialRef}
        color="#00ffff"
        transparent
        opacity={0}
        side={THREE.DoubleSide}
        depthWrite={false}
        blending={THREE.AdditiveBlending}
      />
    </mesh>
  );
}

// Дополнительный компонент: световая вспышка в центре
export function CoreFlash({ trigger = 0, position = [0, 0, 0] }) {
  const sphereRef = useRef();
  const materialRef = useRef();
  const startTime = useRef(0);

  useEffect(() => {
    startTime.current = performance.now() / 1000;
  }, [trigger]);

  useFrame((state) => {
    if (!sphereRef.current || !materialRef.current) return;
    const elapsed = state.clock.getElapsedTime() - startTime.current;
    const duration = 1.5;

    if (elapsed < duration) {
      const progress = elapsed / duration;
      const scale = 0.5 + progress * 3;
      sphereRef.current.scale.setScalar(scale);
      materialRef.current.opacity = Math.max(0, (1 - progress) * 2);
    } else {
      materialRef.current.opacity = 0;
    }
  });

  return (
    <mesh ref={sphereRef} position={position}>
      <sphereGeometry args={[1, 32, 32]} />
      <meshBasicMaterial
        ref={materialRef}
        color="#ffffff"
        transparent
        opacity={0}
        depthWrite={false}
        blending={THREE.AdditiveBlending}
      />
    </mesh>
  );
}