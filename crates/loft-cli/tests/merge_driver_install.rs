//! `loft merge-driver --install` (D-11 F-43) in front of the git simulator: it registers the
//! driver in the repository once, and refuses outside a repository without writing anything.

#[path = "common/schema.rs"]
mod schema;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::{env, fs, process};

use serde_json::{Value, json};
use software_plant::GitScenario;

/// A directory for the tree, and one beside it for the simulator's scenario, removed at the end.
struct Dirs {
    tree: PathBuf,
    plant: PathBuf,
}

impl Dirs {
    fn new(name: &str) -> Self {
        let base = env::temp_dir().join(format!("loft-install-{name}-{}", process::id()));
        let _ = fs::remove_dir_all(&base);
        let dirs = Self {
            tree: base.join("tree"),
            plant: base.join("plant"),
        };
        fs::create_dir_all(dirs.tree.join("spec")).unwrap();
        fs::create_dir_all(&dirs.plant).unwrap();
        dirs
    }

    /// Runs `loft merge-driver --install` with the simulator answering from `scenario`, and
    /// gives its output and the scenario as the run left it.
    fn install(&self, scenario: &GitScenario) -> (Output, GitScenario) {
        self.install_with(scenario, &[])
    }

    /// Runs `loft merge-driver --install` followed by `args`.
    fn install_with(&self, scenario: &GitScenario, args: &[&str]) -> (Output, GitScenario) {
        let installed = scenario.install(&self.plant);
        let mut command = Command::new(env!("CARGO_BIN_EXE_loft"));
        installed.apply(&mut command);
        let output = command
            .args(["merge-driver", "--install"])
            .args(args)
            .current_dir(&self.tree)
            .output()
            .unwrap();
        (output, GitScenario::read(&installed.scenario).unwrap())
    }

    fn attributes(&self) -> Option<String> {
        fs::read_to_string(self.tree.join(".gitattributes")).ok()
    }

    fn files(&self) -> Vec<PathBuf> {
        let mut files: Vec<PathBuf> = walk(&self.tree);
        files.sort();
        files
    }
}

impl Drop for Dirs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(self.tree.parent().unwrap());
    }
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .unwrap()
        .flat_map(|entry| {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path)
            } else {
                vec![path]
            }
        })
        .collect()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap()
}

const DRIVER: &str = "merge.loft.driver";
const NAME: &str = "merge.loft.name";

fn config(scenario: &GitScenario, name: &str) -> Option<String> {
    scenario.repository.as_ref()?.config.get(name).cloned()
}

/// What `loft --version` prints first.
fn own_version() -> String {
    format!("loft {}\n", env!("CARGO_PKG_VERSION"))
}

/// A clean repository on `main` in which `loft` runs where git runs the driver.
fn clean() -> GitScenario {
    GitScenario::clean_on("main").launcher("loft", &own_version())
}

#[test]
fn the_attribute_the_driver_and_its_name_are_registered() {
    let dirs = Dirs::new("fresh");
    let (output, scenario) = dirs.install(&clean());
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(
        text(&output.stdout),
        ".gitattributes\nmerge.loft.driver\nmerge.loft.name\n"
    );
    assert_eq!(
        dirs.attributes().as_deref(),
        Some("spec/**/*.yaml merge=loft\n")
    );
    let driver = config(&scenario, DRIVER).unwrap();
    assert_eq!(driver, loftspec_store::merge_driver::driver("loft"));
    assert!(
        driver.contains("then loft merge-driver %O %A %B %P;"),
        "{driver}"
    );
    assert!(config(&scenario, NAME).is_some_and(|name| !name.is_empty()));
}

#[test]
fn an_existing_attributes_file_keeps_its_lines_and_gains_the_attribute() {
    let dirs = Dirs::new("existing");
    fs::write(dirs.tree.join(".gitattributes"), "*.png binary").unwrap();
    let (output, _) = dirs.install(&clean());
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(
        dirs.attributes().as_deref(),
        Some("*.png binary\nspec/**/*.yaml merge=loft\n")
    );
}

#[test]
fn a_second_install_changes_nothing() {
    let dirs = Dirs::new("again");
    let (_, installed) = dirs.install(&clean());
    let attributes = dirs.attributes();
    let (output, again) = dirs.install(&installed);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(text(&output.stdout), "");
    assert_eq!(again, installed);
    assert_eq!(dirs.attributes(), attributes);
}

#[test]
fn a_driver_registered_with_another_command_is_set_to_loft_s() {
    let dirs = Dirs::new("other");
    let scenario = clean().with_config(DRIVER, "cat %A");
    let (output, after) = dirs.install(&scenario);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert!(text(&output.stdout).contains("merge.loft.driver\n"));
    assert_eq!(
        config(&after, DRIVER),
        Some(loftspec_store::merge_driver::driver("loft"))
    );
}

#[test]
fn the_driver_runs_the_command_given_with_command_and_another_command_replaces_it() {
    let dirs = Dirs::new("command");
    let npx = "npx --no-install loft";
    let scenario = clean().launcher(npx, &own_version());
    let (output, installed) = dirs.install_with(&scenario, &["--command", npx]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let driver = config(&installed, DRIVER).unwrap();
    assert!(
        driver.contains(&format!("then {npx} merge-driver %O %A %B %P;")),
        "{driver}"
    );
    let (output, replaced) = dirs.install(&installed);
    assert_eq!(text(&output.stdout), "merge.loft.driver\n");
    assert_eq!(
        config(&replaced, DRIVER),
        Some(loftspec_store::merge_driver::driver("loft"))
    );
}

#[test]
fn a_command_git_cannot_run_is_refused_and_nothing_is_written() {
    let dirs = Dirs::new("missing");
    let before = dirs.files();
    let scenario = GitScenario::clean_on("main");
    let (output, after) = dirs.install_with(&scenario, &["--command", "pnpm exec loft"]);
    assert_eq!(output.status.code(), Some(1));
    let error = text(&output.stderr);
    assert!(
        error.contains("git could not run `pnpm exec loft --version`"),
        "{error}"
    );
    assert!(error.contains("--command"), "{error}");
    assert_eq!(dirs.files(), before);
    assert_eq!(after, scenario);
}

#[test]
fn a_command_that_starts_another_version_is_warned_of_and_registered() {
    let dirs = Dirs::new("version");
    let scenario = GitScenario::clean_on("main").launcher("loft", "loft 0.1.0\nformat x\n");
    let (output, installed) = dirs.install(&scenario);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let warning = text(&output.stderr);
    assert!(
        warning.contains(&format!(
            "`loft` starts loft 0.1.0, not {}",
            own_version().trim_end()
        )),
        "{warning}"
    );
    assert!(config(&installed, DRIVER).is_some());
}

#[test]
fn outside_a_repository_it_refuses_and_writes_nothing() {
    let dirs = Dirs::new("outside");
    let before = dirs.files();
    let (output, after) = dirs.install(&GitScenario::outside_repository());
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(text(&output.stdout), "");
    assert!(
        text(&output.stderr).contains("a git repository is required"),
        "{}",
        text(&output.stderr)
    );
    assert_eq!(dirs.files(), before);
    assert_eq!(after, GitScenario::outside_repository());
}

#[test]
fn install_takes_no_paths_and_the_paths_need_no_install() {
    let dirs = Dirs::new("usage");
    let output = Command::new(env!("CARGO_BIN_EXE_loft"))
        .args(["merge-driver", "--install", "a", "b", "c", "spec/x.yaml"])
        .current_dir(&dirs.tree)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let output = Command::new(env!("CARGO_BIN_EXE_loft"))
        .args(["merge-driver"])
        .current_dir(&dirs.tree)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}

/// The one JSON document a run printed, held to the `loft merge-driver` schema.
fn json_of(output: &Output) -> Value {
    let document: Value = serde_json::from_str(&text(&output.stdout)).unwrap();
    let schema: Value =
        serde_json::from_str(include_str!("../../../schemas/merge-driver.json")).unwrap();
    assert_eq!(schema::violations(&document, &schema), Vec::<String>::new());
    document
}

#[test]
fn the_json_document_of_an_install_lists_the_names_written_and_the_launcher_s_version() {
    let dirs = Dirs::new("json");
    let (output, _) = dirs.install_with(&clean(), &["--format", "json"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let document = json_of(&output);
    assert_eq!(
        document["written"],
        json!([".gitattributes", "merge.loft.driver", "merge.loft.name"])
    );
    assert_eq!(document["version"], own_version().trim_end());
}

#[test]
fn a_refused_install_is_listed_among_the_json_document_s_errors() {
    let dirs = Dirs::new("json-outside");
    let (output, _) = dirs.install_with(&GitScenario::outside_repository(), &["--format", "json"]);
    assert_eq!(output.status.code(), Some(1));
    let error = &json_of(&output)["errors"][0];
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .contains("a git repository is required"),
        "{error}"
    );
}
