//! The git simulator answers the calls with which `loft status --base` reads the tree at the
//! base commit as its scenario scripts them: the commit `merge-base` gives a revision, the
//! files `ls-tree` lists under `spec/`, and the blobs `cat-file --batch` prints.

use std::io::Write;
use std::process::{Command, Stdio};

use software_plant::GitScenario;
use software_plant::git::Reply;

const BASE: &str = "1f2e3d4c5b6a79881f2e3d4c5b6a79881f2e3d4c";
const LOCK: &str = "aaaa000000000000000000000000000000000001";
const TAGS: &str = "aaaa000000000000000000000000000000000002";

fn args(line: &str) -> Vec<String> {
    line.split(' ').map(str::to_owned).collect()
}

fn answer(scenario: &GitScenario, line: &str) -> Reply {
    scenario.answer(&args(line))
}

/// A clean repository on `feature` whose base for `develop` holds two files under `spec/` and
/// one outside it.
fn based() -> GitScenario {
    GitScenario::clean_on("feature")
        .base("develop", BASE)
        .file_at(
            "develop",
            "README.md",
            "aaaa000000000000000000000000000000000000",
            "x\n",
        )
        .file_at(
            "develop",
            "spec/loftspec.yaml",
            LOCK,
            "format: loftspec/1\n",
        )
        .file_at("develop", "spec/tags.yaml", TAGS, "[]\n")
}

#[test]
fn merge_base_prints_the_base_commit_the_scenario_gives_the_revision() {
    let reply = answer(&based(), "-C /work merge-base HEAD develop");
    assert_eq!(
        (reply.code, reply.stdout),
        (0, format!("{BASE}\n").into_bytes())
    );
}

#[test]
fn merge_base_exits_1_and_prints_nothing_for_a_revision_without_a_common_ancestor() {
    let scenario = GitScenario::clean_on("feature").unrelated("orphan");
    let reply = answer(&scenario, "merge-base HEAD orphan");
    assert_eq!((reply.code, reply.stdout), (1, Vec::new()));
}

#[test]
fn merge_base_exits_128_with_gits_message_for_a_revision_the_scenario_does_not_name() {
    let reply = answer(&based(), "merge-base HEAD nosuch");
    assert_eq!(
        (reply.code, reply.stderr.as_str()),
        (128, "fatal: Not a valid object name nosuch\n")
    );
}

#[test]
fn ls_tree_prints_a_record_for_each_file_under_spec_of_the_base_commit() {
    let reply = answer(&based(), &format!("ls-tree -r -z {BASE} -- spec"));
    let expected =
        format!("100644 blob {LOCK}\tspec/loftspec.yaml\0100644 blob {TAGS}\tspec/tags.yaml\0");
    assert_eq!((reply.code, reply.stdout), (0, expected.into_bytes()));
}

#[test]
fn ls_tree_takes_its_pathspec_and_prints_its_paths_from_the_working_directory() {
    let scenario = GitScenario::clean_on("feature")
        .below("sub/")
        .base("develop", BASE)
        .file_at("develop", "spec/tags.yaml", TAGS, "[]\n")
        .file_at("develop", "sub/spec/tags.yaml", LOCK, "[]\n");
    let reply = answer(&scenario, &format!("ls-tree -r -z {BASE} -- spec"));
    let expected = format!("100644 blob {LOCK}\tspec/tags.yaml\0");
    assert_eq!(reply.stdout, expected.into_bytes());
}

#[test]
fn cat_file_batch_prints_each_blob_it_reads_after_a_header_of_its_id_type_and_size() {
    let input = format!("{LOCK}\n{TAGS}\nbbbb000000000000000000000000000000000000\n");
    let reply = based().answer_with(&args("cat-file --batch"), input.as_bytes());
    let expected = format!(
        "{LOCK} blob 19\nformat: loftspec/1\n\n{TAGS} blob 3\n[]\n\nbbbb000000000000000000000000000000000000 missing\n"
    );
    assert_eq!((reply.code, reply.stdout), (0, expected.into_bytes()));
}

#[test]
fn the_executable_reads_the_object_ids_of_cat_file_batch_on_standard_input() {
    let plant = std::env::temp_dir().join(format!("software-plant-batch-{}", std::process::id()));
    std::fs::create_dir_all(&plant).unwrap();
    let simulator = based().install(&plant);
    let mut child = simulator
        .apply(&mut Command::new("git"))
        .args(["cat-file", "--batch"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(format!("{TAGS}\n").as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    let _ = std::fs::remove_dir_all(&plant);
    assert_eq!(output.stdout, format!("{TAGS} blob 3\n[]\n\n").into_bytes());
}
