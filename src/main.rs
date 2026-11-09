use std::fs;
use std::path::PathBuf;
use std::process::exit;

use clap::{Parser, Subcommand, ValueEnum};
use contract::codegen::{GoGenerator, PythonGenerator, RustGenerator, TypeScriptGenerator};
use contract::diff::{ChangeSeverity, diff};
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

#[derive(Subcommand)]
enum Commands {
    /// Validate a contract definition for syntactic and semantic correctness
    Check {
        /// Path to contract file (.contract, .json, or OpenAPI)
        file: PathBuf,
    },
    /// Compare two versions of a contract and detect breaking changes
    Diff {
        /// Base/Old contract file
        old: PathBuf,
        /// New contract file
        new: PathBuf,
        /// Exit with non-zero code if breaking changes are detected
        #[arg(long, default_value_t = false)]
        fail_on_breaking: bool,
    },
    /// Generate idiomatic client/server code from a contract
    Generate {
        /// Path to contract file
        #[arg(short, long)]
        input: PathBuf,

        /// Target language
        #[arg(short, long, value_enum)]
        lang: TargetLang,

        /// Output file path (prints to stdout if omitted)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Import an existing schema (OpenAPI, Protobuf, GraphQL, JSON Schema) and convert it into a .contract schema
    Import {
        /// Path to schema document (.json, .proto, .graphql, or OpenAPI document)
        input: PathBuf,

        /// Output .contract file (prints to stdout if omitted)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Synchronize external contract dependencies and regenerate client code
    Sync {
        /// Force synchronization even if upstream contains breaking changes
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    /// Check if local contracts have drifted from upstream service definitions
    Drift,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Check { file } => handle_check(file),
        Commands::Diff {
            old,
            new,
            fail_on_breaking,
        } => handle_diff(old, new, fail_on_breaking),
        Commands::Generate {
            input,
            lang,
            output,
        } => handle_generate(input, lang, output),
        Commands::Import { input, output } => handle_import(input, output),
        Commands::Sync { force } => handle_sync(force),
        Commands::Drift => handle_drift(),
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
        match contract::parser::parse(&content) {
            Ok(contract) => contract,
            Err(e) => {
                eprintln!("Error parsing contract DSL '{}': {}", path.display(), e);
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

fn handle_diff(old_path: PathBuf, new_path: PathBuf, fail_on_breaking: bool) {
    let old_contract = read_contract(&old_path);
    let new_contract = read_contract(&new_path);

    let report = diff(&old_contract, &new_contract);

    if report.changes.is_empty() {
        println!("✔ No changes detected between contracts.");
        return;
    }

    println!("Contract Changes ({} total):\n", report.changes.len());

    let mut breaking_count = 0;
    for change in &report.changes {
        let tag = match change.severity {
            ChangeSeverity::Safe => "[SAFE]    ",
            ChangeSeverity::Warning => "[WARNING] ",
            ChangeSeverity::Breaking => {
                breaking_count += 1;
                "[BREAKING]"
            }
        };
        println!("  {} {} -> {}", tag, change.path, change.message);
    }

    println!();
    if breaking_count > 0 {
        eprintln!("✖ Found {} breaking change(s)!", breaking_count);
        if fail_on_breaking {
            exit(1);
        }
    } else {
        println!("✔ All changes are backwards-compatible.");
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
                println!("✔ Artifact '{}' is intact and matches lockfile.", ar.target.out);
            }
            contract::sync::ArtifactDriftStatus::MissingOnDisk => {
                has_breaking = true;
                has_drift = true;
                eprintln!(
                    "✖ Artifact '{}' is missing on disk! Run 'contract sync' to generate.",
                    ar.target.out
                );
            }
            contract::sync::ArtifactDriftStatus::Modified { expected_sha256: _, actual_sha256: _ } => {
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
