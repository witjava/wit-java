use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
use wit_java_core::{config, GenError};

/// WIT → Java declaration generator (implements mapping v1).
#[derive(Parser)]
#[command(name = "wit-java", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// Options shared by `generate` and `check` (spec §11).
#[derive(Args)]
struct MappingArgs {
    /// Prefix for all mapped packages.
    #[arg(long)]
    package_root: Option<String>,
    /// TOML file of package overrides (spec §3.4).
    #[arg(long, value_name = "FILE")]
    package_map: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t = StyleArg::Nested)]
    interface_package_style: StyleArg,
    #[arg(long, default_value = "v1")]
    mapping_version: String,
    #[arg(long)]
    support_package: Option<String>,
    /// Skip writing support sources (assume the published artifact).
    #[arg(long, default_value_t = false)]
    no_support: bool,
    #[arg(long, value_enum, default_value_t = OptionStyleArg::Optional)]
    option_style: OptionStyleArg,
    #[arg(long, value_enum, default_value_t = U64Arg::Long)]
    u64: U64Arg,
    #[arg(long, value_enum, default_value_t = RoleArg::Both)]
    role: RoleArg,
    /// Enable an @unstable feature (repeatable).
    #[arg(long, num_args = 1..)]
    features: Vec<String>,
    /// Enable all @unstable features.
    #[arg(long, default_value_t = false)]
    all_features: bool,
    /// Restrict generation to named worlds (repeatable).
    #[arg(long, num_args = 1..)]
    world: Vec<String>,
}

impl MappingArgs {
    fn to_options(&self) -> Result<config::GenerateOptions, String> {
        let package_map = match &self.package_map {
            Some(path) => {
                let src = std::fs::read_to_string(path)
                    .map_err(|e| format!("cannot read package-map {}: {e}", path.display()))?;
                config::parse_package_map(&src)?
            }
            None => Default::default(),
        };
        let opts = config::GenerateOptions {
            package_root: self.package_root.clone(),
            package_map,
            interface_style: match self.interface_package_style {
                StyleArg::Nested => config::InterfaceStyle::Nested,
                StyleArg::Flat => config::InterfaceStyle::Flat,
            },
            mapping_version: self.mapping_version.clone(),
            support_package: self
                .support_package
                .clone()
                .unwrap_or_else(|| config::DEFAULT_SUPPORT_PACKAGE.to_string()),
            no_support: self.no_support,
            option_style: match self.option_style {
                OptionStyleArg::Optional => config::OptionStyle::Optional,
                OptionStyleArg::Nullable => config::OptionStyle::Nullable,
            },
            u64_style: match self.u64 {
                U64Arg::Long => config::U64Style::Long,
                U64Arg::BigInteger => config::U64Style::BigInteger,
            },
            role: match self.role {
                RoleArg::Host => config::Role::Host,
                RoleArg::Guest => config::Role::Guest,
                RoleArg::Both => config::Role::Both,
            },
            features: self.features.clone(),
            all_features: self.all_features,
            worlds: self.world.clone(),
        };
        opts.validate()?;
        Ok(opts)
    }
}

#[derive(Subcommand)]
enum Command {
    /// Generate Java declarations from WIT.
    Generate {
        /// WIT file or directory (a package with deps/).
        wit_path: PathBuf,
        /// Output directory.
        #[arg(long, required = true)]
        out: PathBuf,
        #[command(flatten)]
        mapping: MappingArgs,
    },
    /// Validate WIT against the mapping without writing files.
    Check {
        /// WIT file or directory (a package with deps/).
        wit_path: PathBuf,
        #[command(flatten)]
        mapping: MappingArgs,
    },
    /// Print the mapping table for a mapping version.
    MappingInfo {
        #[arg(long, default_value = "v1")]
        mapping_version: String,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum StyleArg {
    Nested,
    Flat,
}

#[derive(Clone, Copy, ValueEnum)]
enum OptionStyleArg {
    Optional,
    Nullable,
}

#[derive(Clone, Copy, ValueEnum)]
enum U64Arg {
    Long,
    /// spec §11 spells the value `BigInteger`
    #[value(name = "BigInteger", alias = "big-integer")]
    BigInteger,
}

#[derive(Clone, Copy, ValueEnum)]
enum RoleArg {
    Host,
    Guest,
    Both,
}

fn main() {
    let exit = match Cli::parse().command {
        Command::Generate {
            wit_path,
            out,
            mapping,
        } => {
            let out = Some(out.as_path());
            run(&wit_path, out, &mapping)
        }
        Command::Check { wit_path, mapping } => run(&wit_path, None, &mapping),
        Command::MappingInfo { mapping_version } => {
            if mapping_version != wit_java_core::MAPPING_VERSION {
                eprintln!(
                    "error: unsupported mapping version `{}` (this tool implements `{}`)",
                    mapping_version,
                    wit_java_core::MAPPING_VERSION
                );
                1
            } else {
                print!("{}", wit_java_core::MAPPING_TABLE);
                0
            }
        }
    };
    std::process::exit(exit);
}

fn run(wit_path: &std::path::Path, out: Option<&std::path::Path>, mapping: &MappingArgs) -> i32 {
    let opts = match mapping.to_options() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("error: {e}");
            return 1;
        }
    };

    match wit_java_core::generate(wit_path, &opts) {
        Ok(files) => {
            if let Some(out_dir) = out {
                if let Err(e) = wit_java_core::write_output(&files, out_dir) {
                    eprintln!("error: {e}");
                    return e.exit_code();
                }
                println!("{} files written to {}", files.len(), out_dir.display());
            } else {
                println!("{} files validated", files.len());
            }
            0
        }
        Err(GenError::Diagnostics(diags)) => {
            for d in &diags.0 {
                eprintln!("error[{0}]: {1}", d.code, d.message);
                if let Some(loc) = &d.location {
                    eprintln!("  at {loc}");
                }
            }
            1
        }
        Err(e) => {
            eprintln!("error: {e}");
            e.exit_code()
        }
    }
}
