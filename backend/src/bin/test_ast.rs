#[path = "../agent/ast_verifier.rs"]
mod ast_verifier;

fn main() {
    let patch = include_str!("../agent/test_patch.rs");
    let result = ast_verifier::analyze_ast(patch);

    println!("AST Analysis Result:");
    println!("  has_command_execution: {}", result.has_command_execution);
    println!("  has_unsafe_call: {}", result.has_unsafe_call);
    println!("  has_user_input: {}", result.has_user_input);
}