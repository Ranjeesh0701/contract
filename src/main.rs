use std::fs;
use std::path::PathBuf;
use std::process::exit;

use clap::{Parser, Subcommand, ValueEnum};
use contract::codegen::{GoGenerator, PythonGenerator, RustGenerator, TypeScriptGenerator};
use contract::diff::{ChangeSeverity, DiffDirection, diff_with_direction};
use contract::ir::Contract;

#[derive(Parser)]
#[command(name = "contract", version)]
#[command(
    about = "Cross-repository contract and type-safety platform CLI",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Clone, Copy, ValueEnum)]
enum TargetLang {
    #[value(name = "ts", alias = "typescript")]
    TypeScript,
    #[value(name = "rust", alias = "rs")]
    Rust,
    #[value(name = "python", alias = "py")]
    Python,
    #[value(name = "go", alias = "golang")]
    Go,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
enum DiffFormat {
    Text,
    Json,
    Markdown,
}

#[derive(Subcommand)]
enum Commands {
    Check {
        file: PathBuf,
    },
    Diff {
        old: PathBuf,
        new: PathBuf,
        #[arg(long, default_value_t = false)]
        fail_on_breaking: bool,
        #[arg(short, long, value_enum, default_value = "text")]
        format: DiffFormat,
        #[arg(short, long, value_enum, default_value = "both")]
        direction: DiffDirection,
    },
    Generate {
        #[arg(short, long)]
        input: PathBuf,

        #[arg(short, long, value_enum)]
        lang: TargetLang,

        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    Import {
        input: PathBuf,

        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    Export {
        #[arg(short, long)]
        input: PathBuf,

        #[arg(short, long, default_value = "openapi")]
        format: String,

        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    Sync {
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    Drift,
    Impact {
        old: PathBuf,
        new: Option<PathBuf>,
        #[arg(short, long)]
        target: Option<String>,
        #[arg(short, long, value_enum, default_value = "text")]
        format: DiffFormat,
        #[arg(long, default_value_t = false)]
        fail_on_breaking: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Check { file } => handle_check(file),
        Commands::Diff {
            old,
            new,
            fail_on_breaking,
            format,
            direction,
        } => handle_diff(old, new, fail_on_breaking, format, direction),
        Commands::Generate {
            input,
            lang,
            output,
        } => handle_generate(input, lang, output),
        Commands::Import { input, output } => handle_import(input, output),
        Commands::Export {
            input,
            format,
            output,
        } => handle_export(input, format, output),
        Commands::Sync { force } => handle_sync(force),
        Commands::Drift => handle_drift(),
        Commands::Impact {
            old,
            new,
            target,
            format,
            fail_on_breaking,
        } => handle_impact(old, new, target, format, fail_on_breaking),
    }
}

fn read_contract(path: &PathBuf) -> Contract {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error reading file '{}': {}", path.display(), e);
            exit(1);
        }
    };

    let adapter_reg = contract::adapters::default_registry();
    if let Some(adapter) = adapter_reg.find_adapter(Some(path), &content) {
        match adapter.parse(&content) {
            Ok(contract) => return contract,
            Err(e) => {
                eprintln!(
                    "Error parsing schema document '{}' with adapter '{}': {}",
                    path.display(),
                    adapter.name(),
                    e
                );
                exit(1);
            }
        }
    }

    let is_dsl = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext == "contract")
        .unwrap_or(false)
        || !content.trim_start().starts_with('{');

    if is_dsl {
        match contract::parser::resolve_contract_file(path) {
            Ok(contract) => contract,
            Err(e) => {
                eprintln!("Error resolving contract DSL '{}': {}", path.display(), e);
                exit(1);
            }
        }
    } else {
        match serde_json::from_str::<Contract>(&content) {
            Ok(contract) => contract,
            Err(e) => {
                eprintln!("Error parsing contract JSON '{}': {}", path.display(), e);
                exit(1);
            }
        }
    }
}

fn handle_check(file: PathBuf) {
    let contract = read_contract(&file);

    match contract.validate() {
        Ok(()) => {
            println!(
                "✔ Contract '{}' (v{}) is valid. ({} types, {} services)",
                contract.name,
                contract.version,
                contract.types.len(),
                contract.services.len()
            );
        }
        Err(errors) => {
            eprintln!(
                "✖ Contract '{}' failed validation with {} error(s):\n",
                contract.name,
                errors.len()
            );
            for (idx, err) in errors.iter().enumerate() {
                eprintln!("  {}. {}", idx + 1, err);
            }
            exit(1);
        }
    }
}

fn handle_diff(
    old_path: PathBuf,
    new_path: PathBuf,
    fail_on_breaking: bool,
    format: DiffFormat,
    direction: DiffDirection,
) {
    let old_contract = read_contract(&old_path);
    let new_contract = read_contract(&new_path);

    let report = diff_with_direction(&old_contract, &new_contract, direction);
    let breaking_count = report.breaking_changes().len();
    let warning_count = report.warnings().len();

    match format {
        DiffFormat::Json => {
            let json_output = serde_json::json!({
                "compatible": report.is_compatible(),
                "total_changes": report.changes.len(),
                "breaking_count": breaking_count,
                "warning_count": warning_count,
                "changes": report.changes,
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&json_output).unwrap_or_else(|_| "{}".to_string())
            );
        }
        DiffFormat::Markdown => {
            println!("### Contract Diff Report\n");
            if report.changes.is_empty() {
                println!("✔ No changes detected between contracts.");
            } else {
                let status = if report.has_breaking_changes() {
                    format!(
                        "❌ **Breaking Changes Detected** ({} breaking)",
                        breaking_count
                    )
                } else if report.has_warnings() {
                    format!(
                        "⚠️ **Backwards-Compatible with Warnings** ({} warning(s))",
                        warning_count
                    )
                } else {
                    "✔ **Fully Backwards-Compatible**".to_string()
                };
                println!("**Status:** {}\n", status);
                println!("| Severity | Target Path | Change Description |");
                println!("|---|---|---|");
                for change in &report.changes {
                    let icon = match change.severity {
                        ChangeSeverity::Safe => "🟢 `SAFE`",
                        ChangeSeverity::Warning => "⚠️ `WARNING`",
                        ChangeSeverity::Breaking => "❌ `BREAKING`",
                    };
                    println!("| {} | `{}` | {} |", icon, change.path, change.message);
                }
            }
        }
        DiffFormat::Text => {
            if report.changes.is_empty() {
                println!("✔ No changes detected between contracts.");
                return;
            }

            println!("Contract Changes ({} total):\n", report.changes.len());

            for change in &report.changes {
                let tag = match change.severity {
                    ChangeSeverity::Safe => "[SAFE]    ",
                    ChangeSeverity::Warning => "[WARNING] ",
                    ChangeSeverity::Breaking => "[BREAKING]",
                };
                println!("  {} {} -> {}", tag, change.path, change.message);
            }

            println!();
            if breaking_count > 0 {
                eprintln!("✖ Found {} breaking change(s)!", breaking_count);
            } else if warning_count > 0 {
                println!(
                    "✔ All changes are backwards-compatible ({} deprecation warning(s)).",
                    warning_count
                );
            } else {
                println!("✔ All changes are backwards-compatible.");
            }
        }
    }

    if breaking_count > 0 && fail_on_breaking {
        exit(1);
    }
}

fn handle_impact(
    old_path: PathBuf,
    new_path: Option<PathBuf>,
    target: Option<String>,
    format: DiffFormat,
    fail_on_breaking: bool,
) {
    let old_contract = read_contract(&old_path);

    let report = match new_path {
        Some(ref new_p) => {
            let new_contract = read_contract(new_p);
            contract::diff::analyze_diff_impact(&old_contract, &new_contract, target.as_deref())
        }
        None => {
            if let Some(ref sym) = target {
                contract::diff::analyze_target_impact(&old_contract, sym)
            } else {
                eprintln!(
                    "Error: Either provide a second contract file to compare, or specify --target <symbol> to analyze blast radius."
                );
                exit(1);
            }
        }
    };

    match format {
        DiffFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".to_string())
            );
        }
        DiffFormat::Markdown => {
            println!("### 💥 Contract Impact Blast Radius Report\n");
            let score_level = if report.blast_radius_score >= 20 {
                "🔴 **HIGH**"
            } else if report.blast_radius_score >= 10 {
                "🟡 **MEDIUM**"
            } else {
                "🟢 **LOW**"
            };
            println!(
                "**Blast Radius Score:** {} ({})\n",
                report.blast_radius_score, score_level
            );
            let sev_str = match report.highest_severity {
                Some(ChangeSeverity::Breaking) => "❌ `BREAKING`",
                Some(ChangeSeverity::Warning) => "⚠️ `WARNING`",
                Some(ChangeSeverity::Safe) => "🟢 `SAFE`",
                None => "✔ `NONE`",
            };
            println!("**Highest Severity:** {}", sev_str);
            println!(
                "**Summary:** {} operation(s) across {} service(s) impacted, {} type(s) affected.\n",
                report.total_impacted_operations,
                report.total_impacted_services,
                report.total_affected_types
            );

            if !report.impacted_endpoints.is_empty() {
                println!("| Severity | Impacted Endpoint | Direction | Depth | Blast Radius Path | Cause |");
                println!("|---|---|---|---|---|---|");
                for ep in &report.impacted_endpoints {
                    let icon = match ep.severity {
                        ChangeSeverity::Breaking => "❌ `BREAKING`",
                        ChangeSeverity::Warning => "⚠️ `WARNING`",
                        ChangeSeverity::Safe => "🟢 `SAFE`",
                    };
                    let path_str = ep.path.join(" → ");
                    let cause_str = ep.cause.as_deref().unwrap_or("-");
                    println!(
                        "| {} | `{}.{}` | {} | {} | `{}` | {} |",
                        icon, ep.service, ep.operation, ep.direction, ep.depth, path_str, cause_str
                    );
                }
            } else {
                println!("✔ No endpoints impacted.");
            }

            if !report.affected_types.is_empty() {
                println!(
                    "\n**Affected Models/Types:** `{}`",
                    report.affected_types.join("`, `")
                );
            }
        }
        DiffFormat::Text => {
            println!("========================================");
            println!(" Contract Impact Blast Radius Analysis");
            println!("========================================");
            let score_level = if report.blast_radius_score >= 20 {
                "HIGH"
            } else if report.blast_radius_score >= 10 {
                "MEDIUM"
            } else {
                "LOW"
            };
            println!(
                "Blast Radius Score: {} ({})",
                report.blast_radius_score, score_level
            );
            println!("Impacted Operations: {}", report.total_impacted_operations);
            println!("Impacted Services:   {}", report.total_impacted_services);
            println!("Affected Types:      {}", report.total_affected_types);

            if !report.impacted_endpoints.is_empty() {
                println!("\nImpacted Endpoints:");
                for ep in &report.impacted_endpoints {
                    let icon = match ep.severity {
                        ChangeSeverity::Breaking => "❌ [BREAKING]",
                        ChangeSeverity::Warning => "⚠️ [WARNING]",
                        ChangeSeverity::Safe => "🟢 [SAFE]",
                    };
                    println!(
                        "  {} {}.{} ({}, Depth {})",
                        icon, ep.service, ep.operation, ep.direction, ep.depth
                    );
                    println!("     Path:  {}", ep.path.join(" -> "));
                    if let Some(ref c) = ep.cause {
                        println!("     Cause: {}", c);
                    }
                }
            } else {
                println!("\n✔ No endpoints impacted.");
            }

            if !report.affected_types.is_empty() {
                println!(
                    "\nAffected Models/Types: {}",
                    report.affected_types.join(", ")
                );
            }
        }
    }

    if fail_on_breaking && report.has_breaking_impact() {
        exit(1);
    }
}

fn handle_generate(input: PathBuf, lang: TargetLang, output: Option<PathBuf>) {
    let contract = read_contract(&input);

    if let Err(errors) = contract.validate() {
        eprintln!(
            "✖ Cannot generate code: contract has {} validation error(s):",
            errors.len()
        );
        for err in errors {
            eprintln!("  - {}", err);
        }
        exit(1);
    }

    let code = match lang {
        TargetLang::TypeScript => TypeScriptGenerator::generate(&contract),
        TargetLang::Rust => RustGenerator::generate(&contract),
        TargetLang::Python => PythonGenerator::generate(&contract),
        TargetLang::Go => GoGenerator::generate(&contract),
    };

    match output {
        Some(out_path) => {
            if let Err(e) = fs::write(&out_path, code) {
                eprintln!("Error writing output file '{}': {}", out_path.display(), e);
                exit(1);
            }
            println!("✔ Successfully generated code to '{}'", out_path.display());
        }
        None => {
            print!("{}", code);
        }
    }
}

fn handle_import(input: PathBuf, output: Option<PathBuf>) {
    let content = match fs::read_to_string(&input) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error reading file '{}': {}", input.display(), e);
            exit(1);
        }
    };

    let adapter_reg = contract::adapters::default_registry();
    let (contract, schema_format) =
        if let Some(adapter) = adapter_reg.find_adapter(Some(&input), &content) {
            match adapter.parse(&content) {
                Ok(c) => (c, adapter.name().to_string()),
                Err(e) => {
                    eprintln!(
                        "Error importing document '{}' with adapter '{}': {}",
                        input.display(),
                        adapter.name(),
                        e
                    );
                    exit(1);
                }
            }
        } else {
            match contract::adapters::from_openapi_json(&content) {
                Ok(c) => (c, "OpenAPI".to_string()),
                Err(e) => {
                    eprintln!("Error importing document '{}': {}", input.display(), e);
                    exit(1);
                }
            }
        };

    let dsl = contract::parser::to_dsl(&contract);

    match output {
        Some(out_path) => {
            if let Err(e) = fs::write(&out_path, dsl) {
                eprintln!("Error writing output file '{}': {}", out_path.display(), e);
                exit(1);
            }
            println!(
                "✔ Successfully imported {} to '{}' ({} types, {} services)",
                schema_format,
                out_path.display(),
                contract.types.len(),
                contract.services.len()
            );
        }
        None => {
            print!("{}", dsl);
        }
    }
}

fn handle_export(input: PathBuf, format: String, output: Option<PathBuf>) {
    let contract = read_contract(&input);

    if let Err(errors) = contract.validate() {
        eprintln!(
            "✖ Cannot export: contract has {} validation error(s):",
            errors.len()
        );
        for err in errors {
            eprintln!("  - {}", err);
        }
        exit(1);
    }

    let exported = match format.to_lowercase().as_str() {
        "openapi" | "swagger" | "oas" => match contract::adapters::to_openapi_json(&contract) {
            Ok(json) => json,
            Err(e) => {
                eprintln!("Error exporting to OpenAPI: {}", e);
                exit(1);
            }
        },
        other => {
            eprintln!(
                "✖ Unsupported export format '{}'. Supported: 'openapi'",
                other
            );
            exit(1);
        }
    };

    match output {
        Some(out_path) => {
            if let Err(e) = fs::write(&out_path, exported) {
                eprintln!("Error writing output file '{}': {}", out_path.display(), e);
                exit(1);
            }
            println!(
                "✔ Successfully exported contract to '{}'",
                out_path.display()
            );
        }
        None => {
            println!("{}", exported);
        }
    }
}

fn handle_sync(force: bool) {
    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    match contract::sync::SyncEngine::sync(&current_dir, force) {
        Ok(report) => {
            println!("✔ Successfully synchronized contracts:");
            for dep in &report.synced_contracts {
                println!("  - Locked dependency '{}'", dep);
            }
            println!("\n✔ Generated client/server files:");
            for file in &report.generated_files {
                println!("  - Emitted '{}'", file.display());
            }
        }
        Err(e) => {
            eprintln!("✖ Sync failed: {}", e);
            exit(1);
        }
    }
}

fn handle_drift() {
    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    let dep_reports = match contract::sync::SyncEngine::check_drift(&current_dir) {
        Ok(reports) => reports,
        Err(e) => {
            eprintln!("✖ Dependency drift check failed: {}", e);
            exit(1);
        }
    };

    let artifact_reports = match contract::sync::SyncEngine::check_artifacts(&current_dir) {
        Ok(reports) => reports,
        Err(e) => {
            eprintln!("✖ Artifact drift check failed: {}", e);
            exit(1);
        }
    };

    let mut has_breaking = false;
    let mut has_drift = false;

    for r in &dep_reports {
        match &r.status {
            contract::sync::DriftStatus::UpToDate => {
                println!("✔ Dependency '{}' is up to date.", r.dependency);
            }
            contract::sync::DriftStatus::MissingInLockfile => {
                has_drift = true;
                println!(
                    "⚠ Dependency '{}' is missing in lockfile. Run 'contract sync'.",
                    r.dependency
                );
            }
            contract::sync::DriftStatus::Drifted {
                report,
                is_breaking,
            } => {
                has_drift = true;
                if *is_breaking {
                    has_breaking = true;
                    eprintln!(
                        "✖ Dependency '{}' has drifted with BREAKING changes! ({} breaking)",
                        r.dependency,
                        report.breaking_changes().len()
                    );
                } else {
                    println!(
                        "ℹ Dependency '{}' has safe upstream updates. ({} changes)",
                        r.dependency,
                        report.changes.len()
                    );
                }
            }
        }
    }

    for ar in &artifact_reports {
        match &ar.status {
            contract::sync::ArtifactDriftStatus::UpToDate => {
                println!(
                    "✔ Artifact '{}' is intact and matches lockfile.",
                    ar.target.out
                );
            }
            contract::sync::ArtifactDriftStatus::MissingOnDisk => {
                has_breaking = true;
                has_drift = true;
                eprintln!(
                    "✖ Artifact '{}' is missing on disk! Run 'contract sync' to generate.",
                    ar.target.out
                );
            }
            contract::sync::ArtifactDriftStatus::Modified {
                expected_sha256: _,
                actual_sha256: _,
            } => {
                has_breaking = true;
                has_drift = true;
                eprintln!(
                    "✖ Artifact '{}' was manually modified or tampered! Run 'contract sync' to restore.",
                    ar.target.out
                );
            }
            contract::sync::ArtifactDriftStatus::MissingInLockfile => {
                has_drift = true;
                println!(
                    "⚠ Artifact '{}' is not recorded in lockfile. Run 'contract sync'.",
                    ar.target.out
                );
            }
        }
    }

    if has_breaking {
        exit(1);
    } else if has_drift {
        println!("\nRun 'contract sync' to update local locks and regenerate clients.");
    }
}
