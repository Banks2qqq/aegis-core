use serde::{Deserialize, Serialize};

/// Уровни изоляции
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IsolationLevel {
    /// Стандартный контейнер (Docker/K8s) — мультиарендность
    Low,
    /// Контейнер с песочницей (gVisor/Kata) — защита ядра
    Medium,
    /// MicroVM (Firecracker) — полная изоляция
    High,
    /// Bare metal — максимальная защита для КИИ
    Critical,
}

/// Рабочая нагрузка
#[derive(Debug, Clone)]
pub enum Workload {
    /// Лендинг, статика, публичное API
    Frontend,
    /// Аналитика, обработка логов
    Analytics,
    /// Sentinel-агенты, разведка
    Sentinel,
    /// Deceiver — приманки для хакеров
    Deceiver,
    /// Анализ эксплойтов, 0-day
    ExploitAnalysis,
}

/// Конфигурация изоляции для рабочей нагрузки
#[derive(Debug, Clone)]
pub struct IsolationConfig {
    pub level: IsolationLevel,
    pub runtime: String,
    pub memory_mb: u64,
    pub cpu_cores: f64,
    pub network: NetworkPolicy,
    pub timeout_secs: u64,
}

#[derive(Debug, Clone)]
pub enum NetworkPolicy {
    /// Полный доступ
    Full,
    /// Только исходящие
    OutboundOnly,
    /// Изолированная сеть
    Isolated,
    /// Без сети
    None,
}

/// Адаптивный эшелон изоляции
pub struct AdaptiveIsolation;

impl AdaptiveIsolation {
    /// Выбрать уровень изоляции для рабочей нагрузки
    pub fn for_workload(workload: Workload) -> IsolationConfig {
        match workload {
            Workload::Frontend => IsolationConfig {
                level: IsolationLevel::Low,
                runtime: "docker".into(),
                memory_mb: 256,
                cpu_cores: 1.0,
                network: NetworkPolicy::Full,
                timeout_secs: 300,
            },
            Workload::Analytics => IsolationConfig {
                level: IsolationLevel::Medium,
                runtime: "kata".into(),
                memory_mb: 512,
                cpu_cores: 2.0,
                network: NetworkPolicy::OutboundOnly,
                timeout_secs: 600,
            },
            Workload::Sentinel => IsolationConfig {
                level: IsolationLevel::High,
                runtime: "firecracker".into(),
                memory_mb: 1024,
                cpu_cores: 2.0,
                network: NetworkPolicy::OutboundOnly,
                timeout_secs: 300,
            },
            Workload::Deceiver => IsolationConfig {
                level: IsolationLevel::High,
                runtime: "firecracker".into(),
                memory_mb: 512,
                cpu_cores: 1.0,
                network: NetworkPolicy::Isolated,
                timeout_secs: 3600,
            },
            Workload::ExploitAnalysis => IsolationConfig {
                level: IsolationLevel::Critical,
                runtime: "firecracker".into(),
                memory_mb: 2048,
                cpu_cores: 4.0,
                network: NetworkPolicy::None,
                timeout_secs: 120,
            },
        }
    }

    /// Повысить уровень изоляции при обнаружении угрозы
    pub fn escalate(current: &IsolationConfig, threat_severity: f64) -> IsolationConfig {
        if threat_severity > 0.9 {
            IsolationConfig {
                level: IsolationLevel::Critical,
                runtime: "firecracker".into(),
                memory_mb: current.memory_mb * 2,
                cpu_cores: current.cpu_cores * 2.0,
                network: NetworkPolicy::None,
                timeout_secs: current.timeout_secs / 2,
            }
        } else if threat_severity > 0.7 {
            IsolationConfig {
                level: IsolationLevel::High,
                runtime: "firecracker".into(),
                memory_mb: current.memory_mb,
                cpu_cores: current.cpu_cores,
                network: NetworkPolicy::OutboundOnly,
                timeout_secs: current.timeout_secs,
            }
        } else {
            current.clone()
        }
    }

    /// Для КИИ — принудительная изоляция
    pub fn for_critical_infrastructure(workload: Workload) -> IsolationConfig {
        let mut config = Self::for_workload(workload);
        config.level = IsolationLevel::Critical;
        config.runtime = "firecracker".into();
        config.network = NetworkPolicy::Isolated;
        config
    }
}