use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
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

const GITHUB_REPOSITORY: &str = "tracel-ai/tracel-llvm";

const DEFAULT_BINDINGS_CRATES: &str = "tracel-mlir-sys,tracel-tblgen-rs";

#[derive(clap::Args)]
pub struct BindingsCmdArgs {
    #[command(subcommand)]
    pub cmd: BindingsSubCmd,
}

#[derive(clap::Subcommand)]
pub enum BindingsSubCmd {
    /// Generate Rust bindings for the current platform into the bundle workspace.
    Generate(BindingsGenerateArgs),
    /// Copy generated bindings from the bundle workspace into the matching crates.
    Copy(BindingsCopyArgs),
    /// Download all supported bindings from the GitHub release, then copy them into the matching crates.
    CopyAll(BindingsCopyAllArgs),
    /// Commit generated bindings, push main, then force-update and push the version tag.
    GitUpdate(BindingsGitUpdateArgs),
}

#[derive(clap::Args)]
pub struct BindingsGenerateArgs {
    /// Name of the crates for which we need to generate bindings.
    #[arg(short, long, value_delimiter = ',', default_value = DEFAULT_BINDINGS_CRATES)]
    crates: Vec<String>,
    /// Bundle workspace directory.
    #[arg(long, default_value = ".llvm")]
    workspace_dir: String,
    /// If set then rebuild the bindgen clang workspace from scratch.
    #[arg(long)]
    rebuild: bool,
}

#[derive(clap::Args)]
pub struct BindingsCopyArgs {
    /// Name of the crates for which we need to copy bindings.
    #[arg(short, long, value_delimiter = ',', default_value = DEFAULT_BINDINGS_CRATES)]
    crates: Vec<String>,
    /// Bundle workspace directory.
    #[arg(long, default_value = ".llvm")]
    workspace_dir: String,
}

#[derive(clap::Args)]
pub struct BindingsCopyAllArgs {
    /// Name of the crates for which we need to download and copy bindings.
    #[arg(short, long, value_delimiter = ',', default_value = DEFAULT_BINDINGS_CRATES)]
    crates: Vec<String>,
    /// Bundle workspace directory.
    #[arg(long, default_value = ".llvm")]
    workspace_dir: String,
}

#[derive(clap::Args)]
pub struct BindingsGitUpdateArgs {
    /// Name of the crates for which we need to commit bindings.
    #[arg(short, long, value_delimiter = ',', default_value = DEFAULT_BINDINGS_CRATES)]
    crates: Vec<String>,
    /// Skip confirmation prompt.
    #[arg(short = 'y', long)]
    yes: bool,
}

#[derive(Clone, Copy, Debug)]
struct SupportedPlatform {
    /// Release asset platform stem, matching PlatformTriple::archive_stem().
    ///
    /// Examples:
    /// - linux-x64
    /// - linux-AArch64
    asset_stem: &'static str,
    /// Rust cfg target_os value.
    target_os: &'static str,
    /// Rust cfg target_arch value.
    target_arch: &'static str,
}

const SUPPORTED_PLATFORMS: &[SupportedPlatform] = &[
    SupportedPlatform {
        asset_stem: "linux-x64",
        target_os: "linux",
        target_arch: "x86_64",
    },
    SupportedPlatform {
        asset_stem: "linux-AArch64",
        target_os: "linux",
        target_arch: "aarch64",
    },
    SupportedPlatform {
        asset_stem: "macos-AArch64",
        target_os: "macos",
        target_arch: "aarch64",
    },
    SupportedPlatform {
        asset_stem: "windows-x64",
        target_os: "windows",
        target_arch: "x86_64",
    },
];

pub(crate) fn handle_command(args: BindingsCmdArgs, env: Environment) -> anyhow::Result<()> {
    match args.cmd {
        BindingsSubCmd::Generate(args) => generate_bindings(args, &env),
        BindingsSubCmd::Copy(args) => copy_bindings(args),
        BindingsSubCmd::CopyAll(args) => copy_all_bindings(args, &env),
        BindingsSubCmd::GitUpdate(args) => git_update_bindings(args),
    }
}

fn generate_bindings(args: BindingsGenerateArgs, env: &Environment) -> anyhow::Result<()> {
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
            "tracel-mlir-sys" => {
                generators::mlir_sys::bindgen(&member, &ws)?;
            }
            "tracel-tblgen-rs" => {
                generators::tblgen_sys::bindgen(&member, &ws)?;
            }
            other => {
                group_info!("Skip '{other}' (no bindgen recipe configured)");
                endgroup!();
            }
        }
    }

    if should_copy_after_generate(env) {
        group_info!("Bindings: development environment detected, copying generated bindings");
        copy_current_platform_bindings(crates, &ws)?;
        endgroup!();
    }

    Ok(())
}

fn copy_bindings(args: BindingsCopyArgs) -> anyhow::Result<()> {
    let crates = &args.crates;
    let ws = BundleWorkspace::new(Path::new(&args.workspace_dir))?;

    copy_current_platform_bindings(crates, &ws)
}

fn copy_all_bindings(args: BindingsCopyAllArgs, env: &Environment) -> anyhow::Result<()> {
    ensure_git_worktree_is_clean()?;

    let crates = &args.crates;
    let ws = BundleWorkspace::new(Path::new(&args.workspace_dir))?;

    fs::create_dir_all(&ws.workspace_dir).with_context(|| {
        format!(
            "Should create workspace dir '{}'",
            ws.workspace_dir.display()
        )
    })?;

    for crate_name in crates {
        for platform in SUPPORTED_PLATFORMS {
            let asset_name = bindings_asset_name(platform.asset_stem, crate_name);
            let asset_path = ws.workspace_dir.join(&asset_name);

            if asset_path.exists() {
                continue;
            }

            let url = bindings_release_url(&asset_name);
            download_to_path(&url, &asset_path)
                .with_context(|| format!("Should download bindings asset from {url}"))?;
        }
    }

    copy_bindings_for_platforms(crates, &ws, SUPPORTED_PLATFORMS)?;
    run_fix_lint_and_format(env)?;

    Ok(())
}

fn copy_bindings_for_platforms(
    crates: &[String],
    ws: &BundleWorkspace,
    platforms: &[SupportedPlatform],
) -> anyhow::Result<()> {
    let members = get_workspace_members(WorkspaceMemberType::Crate);

    for member in members {
        if !crates.contains(&member.name) {
            continue;
        }

        match member.name.as_str() {
            "tracel-mlir-sys" | "tracel-tblgen-rs" => {
                for platform in platforms {
                    copy_bindings_for_platform(&member, ws, *platform)?;
                }

                update_feature_gated_region(&member)?;
            }
            other => {
                group_info!("Skip '{other}' (no bindings copy recipe configured)");
                endgroup!();
            }
        }
    }

    Ok(())
}

fn copy_bindings_for_platform(
    member: &WorkspaceMember,
    ws: &BundleWorkspace,
    platform: SupportedPlatform,
) -> anyhow::Result<()> {
    let crate_name = sanitize_for_filename(&member.name);

    let src_name = bindings_asset_name(platform.asset_stem, &crate_name);
    let src = ws.workspace_dir.join(&src_name);

    if !src.is_file() {
        return Err(anyhow::anyhow!(
            "Cannot find generated bindings asset '{}'.\nMissing: {}",
            src_name,
            src.display()
        ));
    }

    let dst_dir = ensure_bindings_dir(member)?;
    let dst_name = bindings_module_file_name(platform.target_os, platform.target_arch);
    let dst = dst_dir.join(dst_name);

    fs::copy(&src, &dst).with_context(|| {
        format!(
            "Should copy generated bindings from '{}' to '{}'",
            src.display(),
            dst.display()
        )
    })?;

    println!("Copied bindings: {} -> {}", src.display(), dst.display());

    Ok(())
}

fn git_update_bindings(args: BindingsGitUpdateArgs) -> anyhow::Result<()> {
    let tag = version_tag();

    if !args.yes {
        let confirmed = prompt_yes_no(
            &format!(
                "This will commit generated bindings, push the commit to origin/main, \
                 then force-update and push tag '{tag}'. Continue?"
            ),
            false,
        )?;

        if !confirmed {
            println!("Aborted.");
            return Ok(());
        }
    }

    commit_bindings_update(&args.crates)?;
    push_main()?;
    force_update_and_push_version_tag()?;

    Ok(())
}

pub(crate) fn get_bindings_file_path(
    member: &WorkspaceMember,
    ws: &BundleWorkspace,
) -> anyhow::Result<String> {
    let platform = ws.platform.archive_stem();
    let crate_name = sanitize_for_filename(&member.name);

    let filename = bindings_asset_name(&platform, &crate_name);
    let path = ws.workspace_dir.join(filename);

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

        // Retrieve OS and arch.
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

        // Only include pregenerated bindings when we are not in xtask mode.
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

fn bindings_asset_name(platform_stem: &str, crate_name: &str) -> String {
    format!("{platform_stem}.{crate_name}.bindings.rs")
}

fn bindings_module_file_name(target_os: &str, target_arch: &str) -> String {
    format!("bindings_{target_os}_{target_arch}.rs")
}

fn bindings_release_url(asset_name: &str) -> String {
    let version = tracel_llvm_bundler::config::TRACEL_LLVM_VERSION;
    let release_number = tracel_llvm_bundler::config::TRACEL_LLVM_RELEASE_NUMBER;
    let tag = format!("v{version}-{release_number}");

    format!("https://github.com/{GITHUB_REPOSITORY}/releases/download/{tag}/{asset_name}")
}

fn sanitize_for_filename(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect()
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

fn should_copy_after_generate(env: &Environment) -> bool {
    env.name == EnvironmentName::Development
}

fn copy_current_platform_bindings(crates: &[String], ws: &BundleWorkspace) -> anyhow::Result<()> {
    let platform = current_supported_platform(ws);
    copy_bindings_for_platforms(crates, ws, &[platform])
}

fn current_supported_platform(ws: &BundleWorkspace) -> SupportedPlatform {
    SupportedPlatform {
        asset_stem: Box::leak(ws.platform.archive_stem().into_boxed_str()),
        target_os: Box::leak(ws.platform.os.clone().into_boxed_str()),
        target_arch: Box::leak(ws.platform.arch.clone().into_boxed_str()),
    }
}

fn ensure_selector_file(member: &WorkspaceMember) -> anyhow::Result<PathBuf> {
    let selector_path = get_selector_file_path(member)?;

    if !selector_path.exists() {
        let mut content = String::new();
        content.push_str("//! Auto-generated binding selector. Do not edit by hand.\n");
        content.push_str("//! This file is partially managed by xtask.\n");
        content.push('\n');
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
            "Bundle is required for bindings generation.\n\
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

fn download_to_path(url: &str, dest: &Path) -> anyhow::Result<()> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60 * 5))
        .build()
        .with_context(|| "Should create HTTP client")?;

    let mut resp = client
        .get(url)
        .send()
        .with_context(|| format!("Should send GET request to {url}"))?
        .error_for_status()
        .with_context(|| format!("GET {url} should return a successful status"))?;

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Should create parent directory '{}'", parent.display()))?;
    }

    let mut file = fs::File::create(dest)
        .with_context(|| format!("Should create destination file '{}'", dest.display()))?;

    std::io::copy(&mut resp, &mut file)
        .with_context(|| format!("Should write response body to '{}'", dest.display()))?;

    Ok(())
}
fn ensure_git_worktree_is_clean() -> anyhow::Result<()> {
    let output = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .with_context(|| "`git status --porcelain` should execute")?;

    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "`git status --porcelain` should succeed.\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout).trim_end(),
            String::from_utf8_lossy(&output.stderr).trim_end()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.trim().is_empty() {
        return Err(anyhow::anyhow!(
            "Git worktree should be clean before running `bindings copy-all`.\n\
             Refusing to mix generated bindings with existing local changes.\n\n\
             Dirty files:\n{}",
            stdout.trim_end()
        ));
    }

    Ok(())
}

fn run_fix_lint_and_format(env: &Environment) -> anyhow::Result<()> {
    let ctx = Context::Std;

    group_info!("Bindings: run fix lint");
    base_commands::fix::handle_command(
        FixCmdArgs {
            command: Some(FixSubCommand::Lint),
            target: Target::Workspace,
            exclude: vec![],
            only: vec![],
            features: vec![],
            no_default_features: false,
            yes: true,
        },
        env.clone(),
        ctx.clone(),
        Some(true),
    )?;
    endgroup!();

    group_info!("Bindings: run fix format");
    base_commands::fix::handle_command(
        FixCmdArgs {
            command: Some(FixSubCommand::Format),
            target: Target::Workspace,
            exclude: vec![],
            only: vec![],
            features: vec![],
            no_default_features: false,
            yes: true,
        },
        env.clone(),
        ctx,
        Some(true),
    )?;
    endgroup!();

    Ok(())
}

fn commit_bindings_update(crates: &[String]) -> anyhow::Result<()> {
    let members = get_workspace_members(WorkspaceMemberType::Crate);

    let mut staged_anything = false;

    for member in members {
        if !crates.contains(&member.name) {
            continue;
        }

        match member.name.as_str() {
            "tracel-mlir-sys" | "tracel-tblgen-rs" => {
                let bindings_dir = bindings_output_dir(&member);
                let bindings_dir = bindings_dir.to_string_lossy().to_string();

                run_process(
                    "git",
                    &["add", "--", &bindings_dir],
                    None,
                    None,
                    "Should stage generated bindings",
                )?;

                staged_anything = true;
            }
            _ => {}
        }
    }

    if !staged_anything {
        return Err(anyhow::anyhow!(
            "No supported binding crate was selected; nothing was staged."
        ));
    }

    let diff_status = std::process::Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .status()
        .with_context(|| "`git diff --cached --quiet` should execute")?;

    if diff_status.success() {
        return Err(anyhow::anyhow!(
            "No generated binding changes were staged; refusing to create an empty commit."
        ));
    }

    let tag = version_tag();

    run_process(
        "git",
        &[
            "commit",
            "-m",
            &format!("Update generated bindings for {tag}"),
        ],
        None,
        None,
        "Should commit generated bindings",
    )?;

    Ok(())
}

fn force_update_and_push_version_tag() -> anyhow::Result<()> {
    let tag = version_tag();

    run_process(
        "git",
        &["tag", "-f", &tag, "HEAD"],
        None,
        None,
        "Should force-update version tag locally",
    )?;

    run_process(
        "git",
        &["push", "origin", &format!("refs/tags/{tag}"), "--force"],
        None,
        None,
        "Should force-push version tag",
    )?;

    Ok(())
}

fn version_tag() -> String {
    let version = tracel_llvm_bundler::config::TRACEL_LLVM_VERSION;
    let release_number = tracel_llvm_bundler::config::TRACEL_LLVM_RELEASE_NUMBER;

    format!("v{version}-{release_number}")
}

fn push_main() -> anyhow::Result<()> {
    run_process(
        "git",
        &["push", "origin", "HEAD:main"],
        None,
        None,
        "Should push generated bindings commit to origin/main",
    )?;

    Ok(())
}
