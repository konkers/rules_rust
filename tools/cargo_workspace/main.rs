use std::collections::{BTreeMap, HashMap};
use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use anyhow::anyhow;
use clap::Parser;
use crate_specs::{generate_crate_specs, get_crate_specs, CrateSpec};

fn crate_name(spec: &CrateSpec) -> anyhow::Result<String> {
    for tag in &spec.tags {
        if tag.starts_with("crate-name=") {
            return Ok(tag
                .strip_prefix("crate-name=")
                .expect("prefix exists")
                .to_string());
        }
    }

    Ok(spec.display_name.clone())
}

fn crate_features(spec: &CrateSpec) -> anyhow::Result<Vec<String>> {
    let mut features = Vec::new();
    for cfg in &spec.cfg {
        if cfg.starts_with("feature=\"") {
            // FIXME: remove unwraps.
            let feature = cfg
                .strip_prefix("feature=\"")
                .unwrap()
                .strip_suffix("\"")
                .unwrap();

            // An empty feature value is possible and can be common if a
            // conditionally enables features with a `select()`.  Omit
            // these from the features array.
            if !feature.is_empty() {
                features.push(feature.to_string());
            }
        }
    }

    Ok(features)
}

fn generate_features_toml_array(features: &[String]) -> String {
    format!(
        "[{}]",
        features
            .iter()
            .map(|f| format!("\"{f}\""))
            .reduce(|acc, f| [acc, f].join(", "))
            .unwrap()
    )
}

fn is_external_crate(spec: &CrateSpec) -> bool {
    !spec.is_workspace_member
}

fn write_dependency<W: Write>(w: &mut W, dep: &CrateSpec) -> anyhow::Result<()> {
    write!(w, "{} = {{ ", crate_name(dep)?)?;
    if is_external_crate(dep) {
        write!(w, "version = \"{}\"", dep.crate_version)?;
    } else {
        write!(w, "path = \"../{}\"", dep.display_name)?;
    }

    let features = crate_features(dep)?;
    if !features.is_empty() {
        write!(
            w,
            ", features = {}",
            generate_features_toml_array(&features)
        )?;
    }
    writeln!(w, " }}")?;

    Ok(())
}

fn write_crate_cargo_toml<W: Write>(
    w: &mut W,
    crate_specs: &BTreeMap<String, CrateSpec>,
    spec: &CrateSpec,
) -> anyhow::Result<()> {
    let features = crate_features(spec)?;

    writeln!(w, "[package]")?;
    writeln!(w, "name = \"{}\"", spec.display_name)?;
    writeln!(w, "version = \"{}\"", spec.crate_version)?;
    writeln!(w, "edition = \"{}\"", spec.edition)?;
    writeln!(w, "")?;

    // TODO: Sort out paths for non-default workspace directory.
    let path_prefix = "../../";
    match spec.crate_type.as_str() {
        "rlib" => {
            writeln!(w, "[lib]")?;
            writeln!(w, "path = \"{path_prefix}{}\"", spec.root_module)?;
        }
        "proc-macro" => {
            writeln!(w, "[lib]")?;
            writeln!(w, "proc-macro = true")?;
            writeln!(w, "path = \"{path_prefix}{}\"", spec.root_module)?;
        }
        // TODO: support binary crates.
        _ => {
            return Err(anyhow!("Unknown crate type {}", spec.crate_type));
        }
    }

    if !features.is_empty() {
        writeln!(w, "")?;
        writeln!(w, "[features]")?;
        writeln!(w, "default = {}", generate_features_toml_array(&features))?;
        for feature in features {
            writeln!(w, "{feature} = []")?;
        }
    }

    if !spec.deps.is_empty() {
        writeln!(w, "")?;
        writeln!(w, "[dependencies]")?;
        for dep_id in &spec.deps {
            let dep_spec = crate_specs.get(dep_id).ok_or_else(|| {
                anyhow!("{} specifies unknown dependency {dep_id}", spec.crate_id)
            })?;
            write_dependency(w, &dep_spec)?;
        }
    }

    Ok(())
}

fn write_workspace_cargo_toml<W: Write>(
    w: &mut W,
    members: &[String],
    workspace_resolver: u32,
) -> anyhow::Result<()> {
    writeln!(w, "[workspace]")?;
    writeln!(w, "resolver = \"{workspace_resolver}\"")?;
    writeln!(w, "members = [")?;
    for member in members {
        writeln!(w, " \"{member}\",")?;
    }
    writeln!(w, "]")?;
    Ok(())
}

fn generate_cargo_workspace(
    bazel: impl AsRef<Path>,
    workspace: impl AsRef<Path>,
    rules_rust_name: &impl AsRef<str>,
    targets: &[String],
    execution_root: impl AsRef<Path>,
    cargo_workspace_root: impl AsRef<Path>,
    workspace_resolver: u32,
) -> anyhow::Result<()> {
    let crate_specs = get_crate_specs(
        bazel.as_ref(),
        workspace.as_ref(),
        execution_root.as_ref(),
        targets,
        rules_rust_name.as_ref(),
    )?
    .into_iter()
    .map(|spec| (spec.crate_id.clone(), spec))
    .collect::<BTreeMap<String, CrateSpec>>();

    let cargo_workspace_root = cargo_workspace_root.as_ref();
    std::fs::create_dir_all(cargo_workspace_root)?;
    let mut members = Vec::new();
    for (_id, spec) in crate_specs
        .iter()
        .filter(|(_id, spec)| !is_external_crate(spec))
    {
        // Skip bin targets as they are not supported.
        if spec.crate_type == "bin" {
            continue;
        }

        let crate_dir = cargo_workspace_root.join(&spec.display_name);
        std::fs::create_dir_all(&crate_dir)?;
        let mut f = File::create(&crate_dir.join("Cargo.toml"))?;
        write_crate_cargo_toml(&mut f, &crate_specs, spec)?;

        members.push(spec.display_name.clone());
    }

    let mut f = File::create(&cargo_workspace_root.join("Cargo.toml"))?;
    write_workspace_cargo_toml(&mut f, &members, workspace_resolver)?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let config = parse_config()?;

    let workspace_root = config
        .workspace
        .as_ref()
        .expect("failed to find workspace root, set with --workspace");

    let execution_root = config
        .execution_root
        .as_ref()
        .expect("failed to find execution root, is --execution-root set correctly?");

    let cargo_workspace_root = config
        .cargo_workspace_root
        .as_ref()
        .expect("failed to find execution root, is --cargo-workspace-root set correctly?");

    let rules_rust_name = env!("ASPECT_REPOSITORY");

    // Generate the crate specs.
    generate_crate_specs(
        &config.bazel,
        workspace_root,
        rules_rust_name,
        &config.targets,
    )?;

    // Use the generated files to write rust-project.json.
    generate_cargo_workspace(
        &config.bazel,
        workspace_root,
        &rules_rust_name,
        &config.targets,
        execution_root,
        cargo_workspace_root,
        config.workspace_resolver,
    )?;

    Ok(())
}

// Parse the configuration flags and supplement with bazel info as needed.
fn parse_config() -> anyhow::Result<Config> {
    let mut config = Config::parse();

    if config.workspace.is_some() && config.execution_root.is_some() {
        return Ok(config);
    }

    // We need some info from `bazel info`. Fetch it now.
    let mut bazel_info_command = Command::new(&config.bazel);
    bazel_info_command.arg("info");
    if let Some(workspace) = &config.workspace {
        bazel_info_command.current_dir(workspace);
    }

    // Execute bazel info.
    let output = bazel_info_command.output()?;
    if !output.status.success() {
        return Err(anyhow!(
            "Failed to run `bazel info` ({:?}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    // Extract the output.
    let output = String::from_utf8_lossy(output.stdout.as_slice());
    let bazel_info = output
        .trim()
        .split('\n')
        .map(|line| line.split_at(line.find(':').expect("missing `:` in bazel info output")))
        .map(|(k, v)| (k, (v[1..]).trim()))
        .collect::<HashMap<_, _>>();

    if config.workspace.is_none() {
        config.workspace = bazel_info.get("workspace").map(Into::into);
    }
    if config.execution_root.is_none() {
        config.execution_root = bazel_info.get("execution_root").map(Into::into);
    }
    if config.cargo_workspace_root.is_none() {
        config.cargo_workspace_root = Some(
            config
                .workspace
                .clone()
                .expect("Bazel workspace is set")
                .join("cargo"),
        );
    }

    Ok(config)
}

#[derive(Debug, Parser)]
struct Config {
    /// The path to the Bazel workspace directory. If not specified, uses the result of `bazel info workspace`.
    #[clap(long, env = "BUILD_WORKSPACE_DIRECTORY")]
    workspace: Option<PathBuf>,

    /// The path to the Bazel execution root. If not specified, uses the result of `bazel info execution_root`.
    #[clap(long)]
    execution_root: Option<PathBuf>,

    /// The path which to write Cargo.toml files. If not specified, defaults to WORKSPACE/cargo.
    #[clap(long)]
    cargo_workspace_root: Option<PathBuf>,

    /// The path to a Bazel binary
    #[clap(long, default_value = "bazel")]
    bazel: PathBuf,

    /// The resolver version to use for the generated workspace.
    #[clap(long, default_value = "2")]
    workspace_resolver: u32,

    /// Space separated list of target patterns that comes after all other args.
    #[clap(default_value = "@//...")]
    targets: Vec<String>,
}
