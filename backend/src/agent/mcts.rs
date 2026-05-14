use rand::Rng;
use super::critic_agent::CriticAgent;
use super::tool_registry::ToolRegistry;

/// Безопасная реализация MCTS (без unsafe)
/// Arena allocation с индексами + UCT selection
#[derive(Clone)]
struct MctsNode {
    action: String,
    visits: u32,
    value: f64,
    children: Vec<usize>,      // индексы в arena
    untried_actions: Vec<String>,
    #[allow(dead_code)]
    parent: Option<usize>,
}

pub struct MctsEngine {
    pub iterations: u32,
    pub exploration_constant: f64,
    pub max_candidates: usize,
    pub max_depth: usize,
    pub risk_cutoff: f64,
}

impl MctsEngine {
    pub fn new(iterations: u32) -> Self {
        Self {
            iterations,
            exploration_constant: 1.414,
            max_candidates: 16,
            max_depth: 8,
            risk_cutoff: 0.85,
        }
    }

    pub async fn select_best_action(
        &self,
        mission: &str,
        current_context: &str,
        possible_actions: &[String],
        critic: &CriticAgent,
        _registry: &ToolRegistry,
    ) -> (String, f64) {
        if possible_actions.is_empty() {
            return ("final_answer()".to_string(), 0.0);
        }

        // Bound candidate set to avoid O(N) blowups from prompt injection or tool list flooding
        let mut candidates: Vec<String> = possible_actions
            .iter()
            .take(self.max_candidates)
            .cloned()
            .collect();
        if candidates.is_empty() {
            candidates.push("final_answer()".to_string());
        }

        let mut arena: Vec<MctsNode> = Vec::with_capacity(256);
        arena.push(MctsNode {
            action: "root".to_string(),
            visits: 0,
            value: 0.0,
            children: vec![],
            untried_actions: candidates,
            parent: None,
        });

        for _ in 0..self.iterations {
            self.run_iteration(&mut arena, mission, current_context, critic).await;
        }

        // Выбираем лучшего ребёнка root
        let root = &arena[0];
        if let Some(&best_idx) = root.children.iter().max_by(|&&a, &&b| {
            let va = if arena[a].visits > 0 { arena[a].value / arena[a].visits as f64 } else { 0.0 };
            let vb = if arena[b].visits > 0 { arena[b].value / arena[b].visits as f64 } else { 0.0 };
            va.partial_cmp(&vb).unwrap_or(std::cmp::Ordering::Equal)
        }) {
            let best = &arena[best_idx];
            let avg = if best.visits > 0 { best.value / best.visits as f64 } else { 0.0 };
            (best.action.clone(), avg)
        } else {
            (possible_actions[0].clone(), 0.0)
        }
    }

    async fn run_iteration(
        &self,
        arena: &mut Vec<MctsNode>,
        mission: &str,
        context: &str,
        critic: &CriticAgent,
    ) {
        // Selection + path как индексы (безопасно)
        let mut node_idx = 0usize;
        let mut path: Vec<usize> = vec![0];
        let mut depth: usize = 0;

        while depth < self.max_depth
            && !arena[node_idx].untried_actions.is_empty()
            && !arena[node_idx].children.is_empty()
        {
            node_idx = self.select_uct(arena, node_idx);
            path.push(node_idx);
            depth += 1;
        }

        // Expansion
        if depth < self.max_depth && !arena[node_idx].untried_actions.is_empty() {
            let idx = rand::thread_rng().gen_range(0..arena[node_idx].untried_actions.len());
            let action = arena[node_idx].untried_actions.remove(idx);

            let score = critic.evaluate(&action, mission, context).await;
            // Hard cutoff: do not expand extremely risky actions
            if score.security_risk >= self.risk_cutoff || score.verdict == "BLOCK" {
                arena[node_idx].visits += 1;
                return;
            }
            let reward = score.utility - score.security_risk * 0.8;

            let child_idx = arena.len();
            arena.push(MctsNode {
                action,
                visits: 1,
                value: reward,
                children: vec![],
                untried_actions: vec![],
                parent: Some(node_idx),
            });
            arena[node_idx].children.push(child_idx);
            path.push(child_idx);
        }

        // Reward для backprop
        let reward = if let Some(&last) = path.last() {
            if !arena[last].children.is_empty() {
                let child = &arena[arena[last].children[0]];
                child.value
            } else {
                let s = critic.evaluate("explore", mission, context).await;
                s.utility - s.security_risk * 0.5
            }
        } else {
            0.0
        };

        // Безопасный backpropagation
        for idx in path {
            arena[idx].visits += 1;
            arena[idx].value += reward;
        }
    }

    fn select_uct(&self, arena: &[MctsNode], node_idx: usize) -> usize {
        let parent_visits = arena[node_idx].visits as f64;
        let node = &arena[node_idx];

        *node.children.iter().max_by(|&&a, &&b| {
            let ua = if arena[a].visits > 0 {
                (arena[a].value / arena[a].visits as f64)
                    + self.exploration_constant * (parent_visits.ln() / arena[a].visits as f64).sqrt()
            } else {
                f64::INFINITY
            };
            let ub = if arena[b].visits > 0 {
                (arena[b].value / arena[b].visits as f64)
                    + self.exploration_constant * (parent_visits.ln() / arena[b].visits as f64).sqrt()
            } else {
                f64::INFINITY
            };
            ua.partial_cmp(&ub).unwrap_or(std::cmp::Ordering::Equal)
        }).unwrap_or(&node.children[0])
    }
}