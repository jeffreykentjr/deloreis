//! Create a project and import its first authored text document.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use clap::Args;
use dli_lorestore::Lorestore;
use dli_world::{Pack, Severity, check};

use crate::cmd::{self, CmdResult, EXIT_CLEAN};

const MANIFEST: &str =
    "version = 1\nname = \"my-story\"\n\n[documents.world]\nstore = \".deloreis/store\"\n";

/// Arguments for creating an empty project.
#[derive(Debug, Args)]
pub(crate) struct InitArgs {
    /// Project directory to create or initialise.
    pub(crate) directory: PathBuf,
}

/// Arguments for importing the first document.
#[derive(Debug, Args)]
pub(crate) struct ImportArgs {
    /// The source document to import.
    pub(crate) file: PathBuf,
    /// Directory containing the project manifest.
    #[arg(long, default_value = ".")]
    pub(crate) project: PathBuf,
}

pub(crate) fn init(args: &InitArgs) -> CmdResult {
    let root = &args.directory;
    let manifest = root.join("deloreis.toml");
    let store_path = root.join(".deloreis/store");
    if manifest.exists() || root.join(".deloreis").exists() {
        return Err(format!(
            "{} already contains a project or store",
            root.display()
        ));
    }
    fs::create_dir_all(root).map_err(|e| format!("cannot create {}: {e}", root.display()))?;
    // Claim the manifest before creating the store, so an existing project is
    // never overwritten even if another initializer races this one.
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&manifest)
        .map_err(|e| format!("cannot create {}: {e}", manifest.display()))?;
    file.write_all(MANIFEST.as_bytes())
        .map_err(|e| format!("cannot write {}: {e}", manifest.display()))?;
    Lorestore::open(store_path).map_err(|e| format!("cannot initialize store: {e}"))?;
    outln!(
        "Created project at {}. To import a world, run: deloreis import FILE --project '{}'",
        root.display(),
        root.to_string_lossy().replace('\'', "'\\''")
    );
    Ok(EXIT_CLEAN)
}

pub(crate) fn import(args: &ImportArgs) -> CmdResult {
    let manifest = args.project.join("deloreis.toml");
    let text = fs::read_to_string(&manifest)
        .map_err(|e| format!("cannot read {}: {e}", manifest.display()))?;
    let config: toml::Value =
        toml::from_str(&text).map_err(|e| format!("invalid project manifest: {e}"))?;
    if config
        .get("documents")
        .and_then(|v| v.get("world"))
        .and_then(|v| v.get("store"))
        .and_then(toml::Value::as_str)
        != Some(".deloreis/store")
    {
        return Err("import expects the world store created by `deloreis init`".to_owned());
    }
    if config
        .get("documents")
        .and_then(|v| v.get("world"))
        .and_then(|v| v.get("head"))
        .is_some()
    {
        return Err(
            "import requires a world that follows main; remove its pinned head first".to_owned(),
        );
    }
    // Use the same full manifest validation as the server before changing the
    // store, so a successful import always targets a project it can load.
    let project = dli_serve::project::Project::load(&args.project)
        .map_err(|e| format!("invalid project: {e}"))?;
    let supplied_root = fs::canonicalize(&args.project)
        .map_err(|e| format!("cannot resolve project directory: {e}"))?;
    if project.root() != supplied_root {
        return Err(
            "project manifest must resolve within the supplied project directory".to_owned(),
        );
    }
    let files = cmd::load_corpus(std::slice::from_ref(&args.file))?;
    let errors: usize = files
        .iter()
        .flat_map(|file| &file.diagnostics)
        .filter(|d| d.class.is_error())
        .count();
    if errors != 0 {
        return Err(format!(
            "source has {errors} syntax error(s); run `deloreis check` for details"
        ));
    }
    let world = cmd::compile_corpus(&files, "import")?;
    let errors: Vec<_> = check::run(&world)
        .into_iter()
        .filter(|f| f.severity == Severity::Error)
        .collect();
    if !errors.is_empty() {
        return Err(format!(
            "source has {} error finding(s); run `deloreis check` for details",
            errors.len()
        ));
    }
    let documents = cmd::documents(&files);
    let name = documents
        .first()
        .and_then(|d| d.scope.as_ref())
        .map_or("world", |s| s.name.text.as_str())
        .to_owned();
    let root = fs::canonicalize(&args.project)
        .map_err(|e| format!("cannot resolve project directory: {e}"))?;
    let store_path = fs::canonicalize(root.join(".deloreis/store"))
        .map_err(|e| format!("cannot resolve project store: {e}"))?;
    if !store_path.starts_with(&root) {
        return Err("project store must be inside the project directory".to_owned());
    }
    let store = Lorestore::open(store_path).map_err(|e| format!("cannot open store: {e}"))?;
    let expected_ref = store
        .branch_ref(dli_lorestore::MAIN_REF)
        .map_err(|e| format!("cannot inspect store: {e}"))?
        .ok_or_else(|| "project store has no main branch".to_owned())?
        .digest()
        .map_err(|e| format!("cannot inspect store: {e}"))?;
    if store
        .head_pack_name()
        .map_err(|e| format!("cannot inspect store: {e}"))?
        .is_some()
    {
        return Err(
            "this store already contains a world; use the authoring tools to make changes"
                .to_owned(),
        );
    }
    let head = store
        .import_pack_at_ref(&Pack { name, documents }, Some(expected_ref.as_str()))
        .map_err(|e| format!("cannot import world: {e}"))?;
    outln!("Imported {} at {}.", args.file.display(), head);
    Ok(EXIT_CLEAN)
}
