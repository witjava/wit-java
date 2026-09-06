use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
use wit_java_core::{config, GenError};

/// WIT → Java declaration generator (implements mapping v1).
#[derive(Parser)]
#[command(name = "wit-java", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generate Java declarations from WIT.
    Generate {
        /// WIT file or directory (a package with deps/).
        wit_path: PathBuf,
        /// Output directory.
        #[arg(long, required_unless_present_any = ["check_only"])]
        out: Option<PathBuf>,
        /// Validate only; do not write files.
        #[arg(long = "check", default_value_t = false)]
        check_only: bool,
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
        #[arg(long, default_value_t = false)]
        all_features: bool,
        /// Restrict generation to named worlds (repeatable).
        #[arg(long, num_args = 1..)]
        world: Vec<String>,
    },
    /// Validate WIT against the mapping without writing files.
    Check {
        wit_path: PathBuf,
        #[arg(long)]
        package_root: Option<String>,
        #[arg(long, value_name = "FILE")]
        package_map: Option<PathBuf>,
        #[arg(long, value_enum, default_value_t = StyleArg::Nested)]
        interface_package_style: StyleArg,
        #[arg(long, default_value = "v1")]
        mapping_version: String,
        #[arg(long)]
        support_package: Option<String>,
        #[arg(long, default_value_t = false)]
        no_support: bool,
        #[arg(long, value_enum, default_value_t = OptionStyleArg::Optional)]
        option_style: OptionStyleArg,
        #[arg(long, value_enum, default_value_t = U64Arg::Long)]
        u64: U64Arg,
        #[arg(long, value_enum, default_value_t = RoleArg::Both)]
        role: RoleArg,
        #[arg(long, num_args = 1..)]
        features: Vec<String>,
        #[arg(long, default_value_t = false)]
        all_features: bool,
        #[arg(long, num_args = 1..)]
        world: Vec<String>,
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
            check_only,
            package_root,
            package_map,
            interface_package_style,
            mapping_version,
            support_package,
            no_support,
            option_style,
            u64,
            role,
            features,
            all_features,
            world,
        } => run(
            &wit_path,
            out.as_deref().filter(|_| !check_only),
            package_root,
            package_map.as_deref(),
            interface_package_style,
            mapping_version,
            support_package,
            no_support,
            option_style,
            u64,
            role,
            features,
            all_features,
            world,
        ),
        Command::Check {
            wit_path,
            package_root,
            package_map,
            interface_package_style,
            mapping_version,
            support_package,
            no_support,
            option_style,
            u64,
            role,
            features,
            all_features,
            world,
        } => run(
            &wit_path,
            None,
            package_root,
            package_map.as_deref(),
            interface_package_style,
            mapping_version,
            support_package,
            no_support,
            option_style,
            u64,
            role,
            features,
            all_features,
            world,
        ),
        Command::MappingInfo { mapping_version } => {
            if mapping_version != wit_java_core::MAPPING_VERSION {
                eprintln!(
                    "error[WJ0001]: unsupported mapping version `{}` (this tool implements `{}`)",
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

#[allow(clippy::too_many_arguments)]
fn run(
    wit_path: &std::path::Path,
    out: Option<&std::path::Path>,
    package_root: Option<String>,
    package_map: Option<&std::path::Path>,
    style: StyleArg,
    mapping_version: String,
    support_package: Option<String>,
    no_support: bool,
    option_style: OptionStyleArg,
    u64: U64Arg,
    role: RoleArg,
    features: Vec<String>,
    all_features: bool,
    world: Vec<String>,
) -> i32 {
    let package_map_map = match package_map {
        Some(path) => match std::fs::read_to_string(path) {
            Ok(src) => match config::parse_package_map(&src) {
                Ok(m) => Some(m),
                Err(d) => {
                    eprintln!("error: {d}");
                    return 1;
                }
            },
            Err(e) => {
                eprintln!("error: cannot read package-map {}: {e}", path.display());
                return 1;
            }
        },
        None => None,
    };
    let opts = config::GenerateOptions {
        package_root,
        package_map: package_map_map.unwrap_or_default(),
        interface_style: match style {
            StyleArg::Nested => config::InterfaceStyle::Nested,
            StyleArg::Flat => config::InterfaceStyle::Flat,
        },
        mapping_version,
        support_package: support_package
            .unwrap_or_else(|| config::DEFAULT_SUPPORT_PACKAGE.to_string()),
        no_support,
        option_style: match option_style {
            OptionStyleArg::Optional => config::OptionStyle::Optional,
            OptionStyleArg::Nullable => config::OptionStyle::Nullable,
        },
        u64_style: match u64 {
            U64Arg::Long => config::U64Style::Long,
            U64Arg::BigInteger => config::U64Style::BigInteger,
        },
        role: match role {
            RoleArg::Host => config::Role::Host,
            RoleArg::Guest => config::Role::Guest,
            RoleArg::Both => config::Role::Both,
        },
        features,
        all_features,
        worlds: world,
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
