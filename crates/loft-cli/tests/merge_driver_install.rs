//! `loft merge-driver --install` (D-11 F-43) in front of the git simulator: it registers the
//! driver in the repository once, and refuses outside a repository without writing anything.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::{env, fs, process};

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
        let installed = scenario.install(&self.plant);
        let mut command = Command::new(env!("CARGO_BIN_EXE_loft"));
        installed.apply(&mut command);
        let output = command
            .args(["merge-driver", "--install"])
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

#[test]
fn ac_571_ac_572_ac_573_the_attribute_the_driver_and_its_name_are_registered() {
    let dirs = Dirs::new("fresh");
    let (output, scenario) = dirs.install(&GitScenario::clean_on("main"));
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(
        text(&output.stdout),
        ".gitattributes\nmerge.loft.driver\nmerge.loft.name\n"
    );
    assert_eq!(
        dirs.attributes().as_deref(),
        Some("spec/**/*.yaml merge=loft\n")
    );
    assert_eq!(
        config(&scenario, DRIVER).as_deref(),
        Some("loft merge-driver %O %A %B %P")
    );
    assert!(config(&scenario, NAME).is_some_and(|name| !name.is_empty()));
}

#[test]
fn ac_571_an_existing_attributes_file_keeps_its_lines_and_gains_the_attribute() {
    let dirs = Dirs::new("existing");
    fs::write(dirs.tree.join(".gitattributes"), "*.png binary").unwrap();
    let (output, _) = dirs.install(&GitScenario::clean_on("main"));
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(
        dirs.attributes().as_deref(),
        Some("*.png binary\nspec/**/*.yaml merge=loft\n")
    );
}

#[test]
fn ac_574_a_second_install_changes_nothing() {
    let dirs = Dirs::new("again");
    let (_, installed) = dirs.install(&GitScenario::clean_on("main"));
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
    let scenario = GitScenario::clean_on("main").with_config(DRIVER, "cat %A");
    let (output, after) = dirs.install(&scenario);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert!(text(&output.stdout).contains("merge.loft.driver\n"));
    assert_eq!(
        config(&after, DRIVER).as_deref(),
        Some("loft merge-driver %O %A %B %P")
    );
}

#[test]
fn ac_575_ac_576_outside_a_repository_it_refuses_and_writes_nothing() {
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
