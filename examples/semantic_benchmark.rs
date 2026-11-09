//! Run with cargo run --release --example semantic_benchmark.
use gdguard::linter::{GDScriptLinter, LinterConfig};
use gdguard::semantic::SemanticModel;
use std::time::Instant;
fn main() -> Result<(), String> {
    for directory in ["tests/lint/input", "tests/input"] {
        let mut sources = Vec::new();
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_gdscript::LANGUAGE.into())
            .map_err(|error| error.to_string())?;
        for entry in std::fs::read_dir(directory).map_err(|error| error.to_string())? {
            let path = entry.map_err(|error| error.to_string())?.path();
            if path.extension().is_some_and(|ext| ext == "gd") {
                let source = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
                let tree = parser.parse(&source, None).ok_or("parse failed")?;
                sources.push((source, tree));
            }
        }
        let rounds = 100;
        let mut linter = GDScriptLinter::new(LinterConfig::default())?;
        let start = Instant::now();
        for _ in 0..rounds {
            for (source, _) in &sources {
                std::hint::black_box(linter.lint(source, "bench.gd")?);
            }
        }
        let lint = start.elapsed();
        let start = Instant::now();
        for _ in 0..rounds {
            for (source, tree) in &sources {
                std::hint::black_box(SemanticModel::build(tree, source));
            }
        }
        let semantic = start.elapsed();
        println!(
            "{}: {} files; lint {:.3} ms; model {:.3} ms; overhead {:.1}%",
            directory,
            sources.len(),
            lint.as_secs_f64() * 10.0,
            semantic.as_secs_f64() * 10.0,
            semantic.as_secs_f64() / lint.as_secs_f64() * 100.0
        );
    }
    #[cfg(feature = "js-rules")]
    {
        use gdguard::linter::api::Config;
        use gdguard::linter::diagnostic::RuleProvider;
        let config = Config {
            custom_rule_paths: vec![std::path::PathBuf::from("examples/rules")],
            ..Default::default()
        };
        let files = gdguard::linter::custom::find_js_files(&config.custom_rule_paths)?;
        let js = gdguard::linter::script::JsRuleProvider::load(&config, &files, &[])?;
        let mut providers: Vec<Box<dyn RuleProvider>> = vec![Box::new(js)];
        let mut baseline: Vec<Box<dyn RuleProvider>> = vec![Box::new(
            gdguard::linter::custom::YamlRuleProvider::new(Vec::new()),
        )];
        let source = std::fs::read_to_string("tests/e2e/sample_project/scripts/player.gd")
            .map_err(|error| error.to_string())?;
        let mut linter = GDScriptLinter::new(LinterConfig::default())?;
        let rounds = 500;
        let start = Instant::now();
        for _ in 0..rounds {
            std::hint::black_box(linter.lint_with_providers(&source, "bench.gd", &mut baseline)?);
        }
        let base = start.elapsed().as_secs_f64() / f64::from(rounds);
        let start = Instant::now();
        for _ in 0..rounds {
            std::hint::black_box(linter.lint_with_providers(
                &source,
                "bench.gd",
                &mut providers,
            )?);
        }
        let scripted = start.elapsed().as_secs_f64() / f64::from(rounds);
        println!(
            "JS examples: model + lint {:.1} us/file; with 2 JS rules {:.1} us/file; extra {:.1} us/file",
            base * 1e6,
            scripted * 1e6,
            (scripted - base) * 1e6
        );
    }
    Ok(())
}
