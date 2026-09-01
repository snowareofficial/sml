use i_slint_compiler::diagnostics::BuildDiagnostics;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = &args[1];
    let source = std::fs::read_to_string(path).expect("read slint");

    let mut diags = BuildDiagnostics::default();
    let syntax = i_slint_compiler::parser::parse(
        source,
        Some(std::path::Path::new(path)),
        &mut diags,
    );

    // 完整编译（语义检查：属性名、类型、继承等）
    let (_doc, diags, _loader) = i_slint_compiler::compile_syntax_node(
        syntax,
        diags,
        i_slint_compiler::CompilerConfiguration::new(
            i_slint_compiler::generator::OutputFormat::Interpreter,
        ),
    )
    .await;

    let mut ok = true;
    for d in diags.iter() {
        if matches!(d.level(), i_slint_compiler::diagnostics::DiagnosticLevel::Error) {
            ok = false;
        }
        println!("DIAG: {:?} | {}", d.level(), d.message());
    }
    if ok {
        println!("SLINT OK (no errors)");
    } else {
        println!("SLINT HAS ERRORS");
    }
    std::process::exit(if ok { 0 } else { 1 });
}
