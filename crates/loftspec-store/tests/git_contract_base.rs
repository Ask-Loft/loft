//! The git contract for the tree at a base commit, which `loft status --base` reads: each
//! scenario builds a history with real git in a scratch repository and scripts the simulator
//! with the commits and blobs real git made, and the two must answer each call Loft makes alike,
//! byte for byte, and give the repository port the same base and the same files.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::common::Scratch;
use crate::real_git::{git, real_git, reply, simulated};
use loftspec::codec::Files;
use loftspec_store::StoreError;
use loftspec_store::repository::{
    BATCH, GitCommand, LIST_TREE, LOCK_WRITTEN, MERGE_BASE, MergeBase,
};
use software_plant::GitScenario;

/// What real git prints for `args` in `dir`, without its last newline.
fn output(dir: &Path, args: &[&str]) -> Vec<u8> {
    let output = real_git().arg("-C").arg(dir).args(args).output().unwrap();
    assert!(output.status.success(), "git {args:?}");
    output.stdout
}

fn rev(dir: &Path, revision: &str) -> String {
    String::from_utf8(output(dir, &["rev-parse", revision]))
        .unwrap()
        .trim_end()
        .to_owned()
}

/// A history in `dir`: `spec/` below `sub` on `develop`, with a hidden file, a nested file and a
/// file outside `spec/`, and a branch `feature` that left it and changed and added a file.
fn history(dir: &Scratch, sub: &str) {
    dir.put(&format!("{sub}spec/loftspec.yaml"), "format: loftspec/1\n");
    dir.put(
        &format!("{sub}spec/products/a/product.yaml"),
        "name: \"A\"\n",
    );
    dir.put(&format!("{sub}spec/.DS_Store"), "x\n");
    dir.put("README.md", "readme\n");
    git(&dir.0, &["init", "--quiet", "-b", "develop"]);
    git(&dir.0, &["add", "."]);
    git(&dir.0, &["commit", "--quiet", "-m", "start"]);
    git(&dir.0, &["checkout", "--quiet", "-b", "feature"]);
    dir.put(
        &format!("{sub}spec/products/a/product.yaml"),
        "name: \"B\"\n",
    );
    dir.put(&format!("{sub}spec/tags.yaml"), "tags\n");
    git(&dir.0, &["add", "."]);
    git(&dir.0, &["commit", "--quiet", "-m", "feature"]);
}

/// The simulator's scenario for `develop` as real git holds it in `dir`: its merge base with
/// HEAD and every file of that commit, each path from the top level.
fn scripted(dir: &Path, prefix: &str) -> GitScenario {
    let commit = rev(dir, "develop");
    let mut scenario = GitScenario::clean_on("feature")
        .below(prefix)
        .base("develop", &commit);
    let listed = output(dir, &["ls-tree", "-r", "-z", "--full-tree", &commit]);
    for record in listed
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        let record = String::from_utf8(record.to_vec()).unwrap();
        let (header, path) = record.split_once('\t').unwrap();
        let object = header.split(' ').nth(2).unwrap();
        let text = String::from_utf8(output(dir, &["cat-file", "blob", object])).unwrap();
        scenario = scenario.file_at("develop", path, object, &text);
    }
    scenario
}

/// One call with `input` on its standard input.
fn run(
    mut command: Command,
    root: &Path,
    call: &[&str],
    input: &str,
) -> (Option<i32>, Vec<u8>, String) {
    let mut child = command
        .arg("-C")
        .arg(root)
        .args(call)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    reply(&child.wait_with_output().unwrap())
}

/// The calls of a read of `develop`'s tree, each with its input.
fn calls(commit: &str, objects: &str) -> Vec<(Vec<String>, String)> {
    let owned = |call: &[&str]| call.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>();
    let mut merge_base = owned(&MERGE_BASE);
    merge_base.push("develop".to_owned());
    let mut list_tree = owned(&LIST_TREE);
    list_tree.extend([commit.to_owned(), "--".to_owned(), "spec".to_owned()]);
    vec![
        (merge_base, String::new()),
        (list_tree, String::new()),
        (owned(&BATCH), objects.to_owned()),
    ]
}

fn root_of(dir: &Scratch, sub: &str) -> PathBuf {
    dir.0.join(sub)
}

#[test]
fn the_simulator_answers_each_call_of_a_base_read_as_real_git_does() {
    for sub in ["", "sub/"] {
        let repository = Scratch::new();
        history(&repository, sub);
        let root = root_of(&repository, sub);
        let commit = rev(&repository.0, "develop");
        let listed = output(&root, &["ls-tree", "-r", "-z", &commit, "--", "spec"]);
        let mut objects = String::new();
        for record in listed
            .split(|byte| *byte == 0)
            .filter(|record| !record.is_empty())
        {
            let record = String::from_utf8_lossy(record);
            let (header, _) = record.split_once('\t').unwrap();
            objects.push_str(header.split(' ').nth(2).unwrap());
            objects.push('\n');
        }
        let plant = Scratch::new();
        let simulator = scripted(&repository.0, sub).install(&plant.0);
        for (call, input) in calls(&commit, &objects) {
            let call: Vec<&str> = call.iter().map(String::as_str).collect();
            let real = run(real_git(), &root, &call, &input);
            let mut command = Command::new("git");
            simulator.apply(&mut command);
            let simulated = run(command, &root, &call, &input);
            assert_eq!(simulated, real, "{sub:?}: git {call:?}");
        }
    }
}

/// What the port reads of `revision` in `root`: its base, and that base's files.
fn base_read(
    git: &GitCommand,
    root: &Path,
    revision: &str,
) -> Result<(MergeBase, Option<Files>), String> {
    let base = git
        .merge_base(root, revision)
        .map_err(|error| error.to_string())?;
    let files = match &base {
        MergeBase::Commit(commit) => Some(
            git.spec_files_at(root, commit)
                .map_err(|error| error.to_string())?,
        ),
        MergeBase::Unrelated => None,
    };
    Ok((base, files))
}

#[test]
fn the_port_reads_the_same_base_and_files_from_the_simulator_and_real_git() {
    for sub in ["", "sub/"] {
        let repository = Scratch::new();
        history(&repository, sub);
        let root = root_of(&repository, sub);
        let plant = Scratch::new();
        let simulator = simulated(&scripted(&repository.0, sub), &plant.0);
        let git = GitCommand::from_command(real_git);
        let read = base_read(&git, &root, "develop").unwrap();
        assert_eq!(
            base_read(&simulator, &root, "develop").unwrap(),
            read,
            "{sub:?}"
        );
        let files = read.1.unwrap();
        let paths: Vec<&str> = files
            .keys()
            .map(loftspec::codec::SpecPath::as_str)
            .collect();
        assert_eq!(
            paths,
            ["loftspec.yaml", "products/a/product.yaml"],
            "{sub:?}"
        );
    }
}

#[test]
fn a_revision_that_names_no_commit_is_refused_by_both() {
    let repository = Scratch::new();
    history(&repository, "");
    let plant = Scratch::new();
    let simulator = simulated(&scripted(&repository.0, ""), &plant.0);
    let real = GitCommand::from_command(real_git);
    for git in [&real, &simulator] {
        let refused = git.merge_base(&repository.0, "nosuch");
        assert!(
            matches!(&refused, Err(StoreError::UnknownRevision(revision)) if revision == "nosuch"),
            "{refused:?}"
        );
    }
}

#[test]
fn a_revision_that_shares_no_history_with_head_is_unrelated_for_both() {
    let repository = Scratch::new();
    history(&repository, "");
    git(
        &repository.0,
        &["checkout", "--quiet", "--orphan", "orphan"],
    );
    git(&repository.0, &["commit", "--quiet", "-m", "orphan"]);
    git(&repository.0, &["checkout", "--quiet", "feature"]);
    let plant = Scratch::new();
    let scenario = GitScenario::clean_on("feature").unrelated("orphan");
    let simulator = simulated(&scenario, &plant.0);
    let real = GitCommand::from_command(real_git);
    for git in [&real, &simulator] {
        assert_eq!(
            git.merge_base(&repository.0, "orphan").unwrap(),
            MergeBase::Unrelated
        );
    }
}

#[test]
fn the_simulator_names_the_commit_that_last_wrote_the_lock_as_real_git_does() {
    for sub in ["", "sub/"] {
        let repository = Scratch::new();
        history(&repository, sub);
        let root = root_of(&repository, sub);
        let call: Vec<&str> = LOCK_WRITTEN.into_iter().chain(["loft.lock.json"]).collect();
        let plant = Scratch::new();
        let unwritten = scripted(&repository.0, sub).install(&plant.0);
        let mut command = Command::new("git");
        unwritten.apply(&mut command);
        let real = run(real_git(), &root, &call, "");
        assert_eq!(real, (Some(0), Vec::new(), String::new()), "{sub:?}");
        assert_eq!(run(command, &root, &call, ""), real, "{sub:?}");

        repository.put(&format!("{sub}loft.lock.json"), "{}\n");
        git(&repository.0, &["add", "."]);
        git(&repository.0, &["commit", "--quiet", "-m", "publish"]);
        let published = rev(&repository.0, "HEAD");
        repository.put(&format!("{sub}spec/tags.yaml"), "more tags\n");
        git(&repository.0, &["add", "."]);
        git(&repository.0, &["commit", "--quiet", "-m", "after"]);
        let plant = Scratch::new();
        let written = scripted(&repository.0, sub)
            .lock_written_at(&published)
            .install(&plant.0);
        let mut command = Command::new("git");
        written.apply(&mut command);
        let real = run(real_git(), &root, &call, "");
        assert_eq!(real.1, format!("{published}\n").into_bytes(), "{sub:?}");
        assert_eq!(run(command, &root, &call, ""), real, "{sub:?}");
    }
}

#[test]
fn the_port_reads_the_same_lock_writer_from_the_simulator_and_real_git() {
    let repository = Scratch::new();
    history(&repository, "");
    let real = GitCommand::from_command(real_git);
    let plant = Scratch::new();
    let simulator = simulated(&scripted(&repository.0, ""), &plant.0);
    assert_eq!(real.lock_written(&repository.0).unwrap(), None);
    assert_eq!(simulator.lock_written(&repository.0).unwrap(), None);
    repository.put("loft.lock.json", "{}\n");
    git(&repository.0, &["add", "."]);
    git(&repository.0, &["commit", "--quiet", "-m", "publish"]);
    let published = rev(&repository.0, "HEAD");
    let plant = Scratch::new();
    let scenario = scripted(&repository.0, "").lock_written_at(&published);
    let simulator = simulated(&scenario, &plant.0);
    assert_eq!(
        real.lock_written(&repository.0).unwrap(),
        Some(published.clone())
    );
    assert_eq!(
        simulator.lock_written(&repository.0).unwrap(),
        Some(published)
    );
}
