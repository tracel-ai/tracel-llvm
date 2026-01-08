use std::{
    fs,
    path::{Path, PathBuf},
};

use tracel_xtask::{
    prelude::{anyhow::Context as _, *},
    utils::workspace::{WorkspaceMember, WorkspaceMemberType, get_workspace_members},
};

use crate::commands::bundle::{BundleBuildArgs, BundleCmdArgs, BundleSubCmd};

use super::{BundleWorkspace, generators};

const FEATURE_GATED_REGION_BEGIN: &str = "// BEGIN AUTO-GENERATED FEATURE GATED REGION";
const FEATURE_GATED_REGION_END: &str = "// END AUTO-GENERATED FEATURE GATED BINDINGS";
const FEATURE_GATE: &str = "xtask";

#[derive(clap::Args)]
pub struct BindgenCmdArgs {
    /// Name of the crates for which we need to generate bindings.
    #[arg(short, long, value_delimiter = ',', default_value = "tracel-mlir-sys,tracel-tblgen-rs")]
    crates: Vec<String>,

    /// Bundle workspace directory.
    #[arg(long, default_value = ".llvm")]
    workspace_dir: String,

    /// If set then rebuild the workspace from scratch
    #[arg(long)]
    rebuild: bool,
}

pub(crate) fn handle_command(args: BindgenCmdArgs) -> anyhow::Result<()> {
    let crates = &args.crates;
    let ws = BundleWorkspace::new(Path::new(&args.workspace_dir))?;
    ensure_bundle_is_built(&ws)?;
    ws.build_clang_for_bindgen(args.rebuild)?;

    // Apply the env your ecosystem expects.
    let major = llvm_major_version()?;
    apply_env_vars(&ws, major);

    let members = get_workspace_members(WorkspaceMemberType::Crate);
    for member in members {
        if !crates.contains(&member.name) {
            continue;
        }
        match member.name.as_str() {
            "tracel-mlir-sys" => generators::mlir_sys::bindgen(&member, &ws)?,
            "tracel-tblgen-rs" => generators::tblgen_sys::bindgen(&member, &ws)?,
            other => {
                group_info!("Skip '{other}' (no bindgen recipe configured)");
                endgroup!();
            }
        }
    }

    Ok(())
}

pub(crate) fn get_bindings_file_path(member: &WorkspaceMember) -> anyhow::Result<String> {
    let out_path = get_output_path(member)?;
    let platform = platform_suffix_for_feature();
    let filename = format!("bindings_{}.rs", platform);
    let path = out_path.join(filename);
    Ok(path.to_string_lossy().into_owned())
}

pub(crate) fn get_wrapper_file_path(member: &WorkspaceMember) -> anyhow::Result<String> {
    let out_path = get_input_path(member)?;
    let path = out_path.join("wrapper.h");
    Ok(path.to_string_lossy().into_owned())
}

/// Applies environment variables expected by sys crates.
///
/// - MLIR_SYS_<major>0_PREFIX and TABLEGEN_<major>0_PREFIX point to the MLIR bundle install.
/// - LIBCLANG_PATH points to the clang bindgen toolchain libdir.
pub(crate) fn apply_env_vars(ws: &BundleWorkspace, major: usize) {
    let mlir_prefix = ws.bundle_install_dir.as_os_str();

    unsafe {
        std::env::set_var(format!("MLIR_SYS_{major}0_PREFIX"), mlir_prefix);
        std::env::set_var(format!("TABLEGEN_{major}0_PREFIX"), mlir_prefix);
        std::env::set_var("LIBCLANG_PATH", ws.clang_lib_dir.as_os_str());
    }
}

pub(crate) fn llvm_major_version() -> anyhow::Result<usize> {
    let major = tracel_llvm_bundler::config::TRACEL_LLVM_VERSION
        .split('.')
        .next()
        .ok_or_else(|| anyhow::anyhow!("TRACEL_LLVM_VERSION should have a major version"))?
        .parse::<usize>()
        .with_context(|| "LLVM major version should parse")?;
    Ok(major)
}

pub(crate) fn update_feature_gated_region(member: &WorkspaceMember) -> anyhow::Result<()> {
    let selector_path = ensure_selector_file(member)?;
    let bindings_dir = ensure_bindings_dir(member)?;

    // An entry is the tuple (module_name, os, arch)
    let mut entries: Vec<(String, String, String)> = Vec::new();

    for entry in fs::read_dir(&bindings_dir).expect("Should read bindings directory") {
        let entry = entry.expect("Should read directory entry");
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name,
            None => continue,
        };

        if !name.starts_with("bindings_") || !name.ends_with(".rs") {
            continue;
        }

        // Module name: "bindings_<os>_<arch>.rs"
        let stem = &name["bindings_".len()..name.len() - ".rs".len()];

        // Retrieve OS and arch
        let (os, arch) = match stem.split_once('_') {
            Some((os, arch)) => (os.to_string(), arch.to_string()),
            None => continue,
        };

        let module_name = sanitize_for_ident(name.strip_suffix(".rs").unwrap());
        entries.push((module_name, os, arch));
    }

    entries.sort_by(|a, b| a.1.cmp(&b.1).then(a.2.cmp(&b.2)));

    let mut generated = String::new();
    let mut base_conditions: Vec<String> = Vec::new();
    for (module, os, arch) in &entries {
        let base_cond = format!("all(target_os = \"{os}\", target_arch = \"{arch}\")");
        base_conditions.push(base_cond.clone());

        // only include pregenerated bindings when we are not in xtask mode
        let cfg_expr = format!("all(not(feature = \"{FEATURE_GATE}\"), {base_cond})");
        generated.push_str(&format!("#[cfg({cfg_expr})]\n"));
        generated.push_str(&format!("mod {module};\n\n"));
        generated.push_str(&format!("#[cfg({cfg_expr})]\n"));
        generated.push_str(&format!("pub use {module}::*;\n\n"));
    }

    if !entries.is_empty() {
        let joined = base_conditions
            .iter()
            .map(|c| format!("        {c},"))
            .collect::<Vec<_>>()
            .join("\n");
        generated.push_str("#[cfg(all(\n");
        generated.push_str(&format!("    not(feature = \"{FEATURE_GATE}\"),\n"));
        generated.push_str("    not(any(\n");
        generated.push_str(&joined);
        generated.push_str("\n    )),\n");
        generated.push_str("))]\n");
        generated.push_str(
            "compile_error!(\"No pre-generated bindings available for this target_os/target_arch combination.\");\n",
        );
    } else {
        generated.push_str(&format!("#[cfg(not(feature = \"{FEATURE_GATE}\"))]\n"));
        generated.push_str(
            "compile_error!(\"No generated bindings modules were found in src/bindings.\");\n",
        );
    }

    let existing = fs::read_to_string(&selector_path).expect("Should read selector file");
    let begin_idx = existing
        .find(FEATURE_GATED_REGION_BEGIN)
        .ok_or_else(|| anyhow::anyhow!("Should find FEATURE GATED REGION begin marker"))?;
    let end_idx = existing
        .find(FEATURE_GATED_REGION_END)
        .ok_or_else(|| anyhow::anyhow!("Should find FEATURE GATED REGION end marker"))?;

    let before = &existing[..begin_idx + FEATURE_GATED_REGION_BEGIN.len()];
    let after = &existing[end_idx..];

    let mut new_content = String::new();
    new_content.push_str(before);
    new_content.push_str("\n\n");
    new_content.push_str(&generated);
    new_content.push('\n');
    new_content.push_str(after);

    if new_content != existing {
        fs::write(&selector_path, new_content).expect("Should update selector file");
    }

    Ok(())
}

fn get_output_path(member: &WorkspaceMember) -> anyhow::Result<PathBuf> {
    let path = Path::new(&member.path).join("src").join("bindings");
    if path.exists() {
        Ok(path)
    } else {
        Err(anyhow::anyhow!(
            "Cannot find output path: {}",
            path.display()
        ))
    }
}

fn get_input_path(member: &WorkspaceMember) -> anyhow::Result<PathBuf> {
    let path = Path::new(&member.path).to_path_buf();
    if path.exists() {
        Ok(path)
    } else {
        Err(anyhow::anyhow!(
            "Cannot find input path: {}",
            path.display()
        ))
    }
}

fn platform_suffix_for_feature() -> String {
    format!("{}_{}", std::env::consts::OS, std::env::consts::ARCH)
}

fn sanitize_for_ident(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn get_selector_file_path(member: &WorkspaceMember) -> anyhow::Result<PathBuf> {
    let out_path = get_output_path(member)?;
    Ok(out_path.join("mod.rs"))
}

fn bindings_output_dir(member: &WorkspaceMember) -> PathBuf {
    Path::new(&member.path).join("src").join("bindings")
}

fn ensure_bindings_dir(member: &WorkspaceMember) -> anyhow::Result<PathBuf> {
    let dir = bindings_output_dir(member);
    if !dir.exists() {
        fs::create_dir_all(&dir).expect("Should create bindings dir");
    }
    Ok(dir)
}

fn ensure_selector_file(member: &WorkspaceMember) -> anyhow::Result<PathBuf> {
    let selector_path = get_selector_file_path(member)?;

    if !selector_path.exists() {
        let mut content = String::new();
        content.push_str("//! Auto-generated binding selector. Do not edit by hand.\n");
        content.push_str("//! This file is partially managed by xtask.\n");
        content.push_str("\n");
        content.push_str(FEATURE_GATED_REGION_BEGIN);
        content.push('\n');
        content.push_str(FEATURE_GATED_REGION_END);
        content.push('\n');

        fs::write(&selector_path, content).expect("Should create selector file");
    } else {
        let text = fs::read_to_string(&selector_path).expect("Should read selector file");
        if !text.contains(FEATURE_GATED_REGION_BEGIN) || !text.contains(FEATURE_GATED_REGION_END) {
            return Err(anyhow::anyhow!(
                "Selector file {} is missing FEATURE GATED REGION markers",
                selector_path.display()
            ));
        }
    }

    Ok(selector_path)
}

fn prompt_yes_no(question: &str, default_yes: bool) -> anyhow::Result<bool> {
    use std::io::{self, Write as _};

    let suffix = if default_yes { "[Y/n]" } else { "[y/N]" };

    loop {
        print!("{question} {suffix}: ");
        io::stdout().flush().expect("stdout should flush");

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .with_context(|| "stdin read should succeed")?;

        let s = input.trim().to_ascii_lowercase();
        match s.as_str() {
            "" => return Ok(default_yes),
            "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            _ => {
                println!("Please answer 'y' or 'n'.");
            }
        }
    }
}

fn ensure_bundle_is_built(ws: &BundleWorkspace) -> anyhow::Result<()> {
    if ws.bundle_install_dir.exists() {
        return Ok(());
    }

    let do_build = prompt_yes_no(
        &format!(
            "Bundle not found at '{}'. Build it now?",
            ws.bundle_install_dir.display()
        ),
        false,
    )?;

    if !do_build {
        return Err(anyhow::anyhow!(
            "Bundle is required for bindgen.\n\
             Missing: {}",
            ws.bundle_install_dir.display()
        ));
    }

    // If sources are missing, we need to git clone first.
    if !ws.llvm_dir.exists() {
        ws.clone_llvm_project_fresh()?;
    }

    super::bundle::handle_command(BundleCmdArgs {
        cmd: BundleSubCmd::Build(BundleBuildArgs {
            workspace_dir: ws.workspace_dir.to_string_lossy().to_string(),
        }),
    })?;
    Ok(())
}
