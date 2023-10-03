use std::collections::HashMap;
use std::path::Path;

use crate_specs::get_crate_specs;
use runfiles::Runfiles;

mod rust_project;

const SYSROOT_SRC_FILE_RUNFILES_PREFIX: &str = "rules_rust";

pub fn write_rust_project(
    bazel: impl AsRef<Path>,
    workspace: impl AsRef<Path>,
    rules_rust_name: &impl AsRef<str>,
    targets: &[String],
    execution_root: impl AsRef<Path>,
    output_base: impl AsRef<Path>,
    rust_project_path: impl AsRef<Path>,
) -> anyhow::Result<()> {
    let crate_specs = get_crate_specs(
        bazel.as_ref(),
        workspace.as_ref(),
        execution_root.as_ref(),
        targets,
        rules_rust_name.as_ref(),
    )?;

    let workspace_name = match rules_rust_name.as_ref().trim_start_matches('@') {
        "" => SYSROOT_SRC_FILE_RUNFILES_PREFIX,
        s => s,
    };
    let toolchain_info_path = format!(
        "{workspace_name}/rust/private/rust_analyzer_detect_sysroot.rust_analyzer_toolchain.json"
    );
    let r = Runfiles::create()?;
    let path = r.rlocation(toolchain_info_path);
    let toolchain_info: HashMap<String, String> =
        serde_json::from_str(&std::fs::read_to_string(path)?)?;

    let sysroot_src = &toolchain_info["sysroot_src"];
    let sysroot = &toolchain_info["sysroot"];

    let rust_project = rust_project::generate_rust_project(sysroot, sysroot_src, &crate_specs)?;

    rust_project::write_rust_project(
        rust_project_path.as_ref(),
        execution_root.as_ref(),
        output_base.as_ref(),
        &rust_project,
    )?;

    Ok(())
}
