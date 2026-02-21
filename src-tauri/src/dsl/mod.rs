#[allow(
    clippy::indexing_slicing,
    clippy::wildcard_imports,
    clippy::cast_possible_truncation,
    clippy::single_match_else,
    clippy::needless_pass_by_value,
    clippy::module_name_repetitions,
)]
pub mod ast;
#[allow(
    clippy::indexing_slicing,
    clippy::wildcard_imports,
    clippy::cast_possible_truncation,
    clippy::single_match_else,
    clippy::needless_pass_by_value,
    clippy::module_name_repetitions,
)]
pub mod error;
#[allow(
    clippy::indexing_slicing,
    clippy::wildcard_imports,
    clippy::cast_possible_truncation,
    clippy::single_match_else,
    clippy::needless_pass_by_value,
    clippy::module_name_repetitions,
)]
pub mod lexer;
#[allow(
    clippy::indexing_slicing,
    clippy::wildcard_imports,
    clippy::cast_possible_truncation,
    clippy::single_match_else,
    clippy::needless_pass_by_value,
    clippy::module_name_repetitions,
)]
pub mod parser;
#[allow(
    clippy::indexing_slicing,
    clippy::wildcard_imports,
    clippy::cast_possible_truncation,
    clippy::single_match_else,
    clippy::needless_pass_by_value,
    clippy::module_name_repetitions,
)]
pub mod builtins;
#[allow(
    clippy::indexing_slicing,
    clippy::wildcard_imports,
    clippy::cast_possible_truncation,
    clippy::single_match_else,
    clippy::needless_pass_by_value,
    clippy::module_name_repetitions,
)]
pub mod typeck;
#[allow(
    clippy::indexing_slicing,
    clippy::wildcard_imports,
    clippy::cast_possible_truncation,
    clippy::single_match_else,
    clippy::needless_pass_by_value,
    clippy::module_name_repetitions,
)]
pub mod compiler;
#[allow(
    clippy::indexing_slicing,
    clippy::wildcard_imports,
    clippy::cast_possible_truncation,
    clippy::single_match_else,
    clippy::needless_pass_by_value,
    clippy::module_name_repetitions,
)]
pub mod vm;

use compiler::CompiledScript;
use error::CompileError;

/// Compile a DSL source string into a `CompiledScript` ready for VM execution.
///
/// This is the primary public entry point for the DSL pipeline:
/// source → lex → parse → type check → compile → `CompiledScript`
pub fn compile_source(source: &str) -> Result<CompiledScript, Vec<CompileError>> {
    let tokens = lexer::lex(source)?;
    let ast = parser::parse(tokens)?;
    let typed = typeck::type_check(&ast)?;
    compiler::compile(&typed).map_err(|e| vec![e])
}
