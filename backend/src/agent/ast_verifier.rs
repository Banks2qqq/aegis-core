//! AST-based Formal Verification (Phase 4)

use std::collections::HashSet;
use syn::{visit::Visit, Expr, ExprAssign, ItemFn, Local, Pat};

pub struct TaintAnalyzer {
    pub has_user_input: bool,
    pub has_unsafe_call: bool,
    pub has_command_execution: bool,
    tainted_vars: HashSet<String>,
}

impl TaintAnalyzer {
    pub fn new() -> Self {
        Self {
            has_user_input: false,
            has_unsafe_call: false,
            has_command_execution: false,
            tainted_vars: HashSet::new(),
        }
    }
}

impl Default for TaintAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl<'ast> Visit<'ast> for TaintAnalyzer {
    fn visit_local(&mut self, local: &'ast Local) {
        if let Some(init) = &local.init {
            if self.is_tainted_expr(&init.expr) {
                if let Pat::Ident(ident) = &local.pat {
                    self.tainted_vars.insert(ident.ident.to_string());
                }
            }
        }
        syn::visit::visit_local(self, local);
    }

    fn visit_expr_assign(&mut self, assign: &'ast ExprAssign) {
        if let Expr::Path(path) = &*assign.left {
            if let Some(ident) = path.path.get_ident() {
                if self.is_tainted_expr(&assign.right) {
                    self.tainted_vars.insert(ident.to_string());
                }
            }
        }
        syn::visit::visit_expr_assign(self, assign);
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        if let Expr::Call(call) = expr {
            if let Expr::Path(path) = &*call.func {
                let fn_name = path.path.segments.last()
                    .map(|s| s.ident.to_string())
                    .unwrap_or_default();

                // Проверяем, используется ли tainted переменная
                for arg in &call.args {
                    if self.is_tainted_expr(arg)
                        && (fn_name.contains("Command")
                            || fn_name.contains("system")
                            || fn_name.contains("exec"))
                    {
                        self.has_command_execution = true;
                    }
                }
            }
        }

        if let Expr::Unsafe(_unsafe_block) = expr {
            self.has_unsafe_call = true;
        }

        syn::visit::visit_expr(self, expr);
    }
}

impl TaintAnalyzer {
    fn is_tainted_expr(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Path(path) => {
                if let Some(ident) = path.path.get_ident() {
                    self.tainted_vars.contains(&ident.to_string())
                } else {
                    false
                }
            }
            Expr::Call(call) => {
                if let Expr::Path(path) = &*call.func {
                    let fn_name = path.path.segments.last()
                        .map(|s| s.ident.to_string())
                        .unwrap_or_default();
                    fn_name.contains("from") || fn_name.contains("parse")
                } else {
                    false
                }
            }
            _ => false,
        }
    }
}

/// Анализирует патч на наличие опасных паттернов через AST
pub fn analyze_ast(patch: &str) -> TaintAnalyzer {
    let mut analyzer = TaintAnalyzer::new();

    // Пытаемся распарсить как Rust функцию
    if let Ok(item) = syn::parse_str::<ItemFn>(patch) {
        analyzer.visit_item_fn(&item);
    } else {
        // Если не получилось — анализируем как обычный текст (fallback)
        if patch.contains("Command::new") || patch.contains("system(") || patch.contains("exec(") {
            analyzer.has_command_execution = true;
        }
        if patch.contains("unsafe") {
            analyzer.has_unsafe_call = true;
        }
    }

    analyzer
}