//! The git simulator answers each call Loft makes as its scenario scripts it, fails as git
//! fails where the scenario says so, and refuses every other call.

use std::process::Command;

use software_plant::GitScenario;
use software_plant::git::{
    Call, LOCAL_OUTSIDE_A_REPOSITORY, NOT_A_REPOSITORY, OPTIONAL_LOCKS_ENV, Reply, UNSCRIPTED,
};

fn args(line: &str) -> Vec<String> {
    line.split(' ').map(str::to_owned).collect()
}

const STATUS: &str = "-C /work status --porcelain=v1 -z --untracked-files=all -- spec";

fn answer(scenario: &GitScenario, line: &str) -> Reply {
    scenario.answer(&args(line))
}

#[test]
fn ac_965_the_prefix_is_answered_as_rev_parse_prints_it() {
    let top = GitScenario::clean_on("main");
    assert_eq!(answer(&top, "rev-parse --show-prefix").stdout, b"\n");
    let below = top.below("sub/");
    assert_eq!(
        answer(&below, "-C /work rev-parse --show-prefix").stdout,
        b"sub/\n"
    );
}

#[test]
fn ac_966_the_branch_is_answered_as_symbolic_ref_prints_it() {
    let reply = answer(
        &GitScenario::clean_on("main"),
        "symbolic-ref --quiet --short HEAD",
    );
    assert_eq!((reply.code, reply.stdout), (0, b"main\n".to_vec()));
}

#[test]
fn ac_972_a_detached_head_exits_1_and_prints_nothing() {
    let reply = answer(
        &GitScenario::clean_on("main").detached(),
        "symbolic-ref --quiet --short HEAD",
    );
    assert_eq!(
        reply,
        Reply {
            code: 1,
            stdout: Vec::new(),
            stderr: String::new(),
            optional_lock: None,
            set: None,
        }
    );
}

#[test]
fn ac_967_the_status_is_answered_as_porcelain_v1_prints_it() {
    let scenario = GitScenario::clean_on("main")
        .with("??", "spec/x/y")
        .with(" M", "spec/a")
        .with("D ", "spec/z")
        .with("??", "spec/new")
        .renamed("spec/c", "spec/c2")
        .with(" M", "README.md");
    let reply = answer(&scenario, STATUS);
    assert_eq!(
        reply.stdout,
        b" M spec/a\0R  spec/c2\0spec/c\0D  spec/z\0?? spec/new\0?? spec/x/y\0".to_vec()
    );
}

#[test]
fn ac_967_the_status_takes_its_pathspec_from_the_working_directory() {
    let scenario = GitScenario::clean_on("main")
        .below("docs/")
        .with(" M", "docs/spec/a")
        .with(" M", "spec/b")
        .with(" M", "docs/specs/c");
    assert_eq!(
        answer(&scenario, STATUS).stdout,
        b" M docs/spec/a\0".to_vec()
    );
}

#[test]
fn ac_971_outside_a_repository_each_call_exits_128_with_gits_message() {
    let scenario = GitScenario::outside_repository();
    for line in [
        "rev-parse --show-prefix",
        "symbolic-ref --quiet --short HEAD",
        STATUS,
    ] {
        let reply = answer(&scenario, line);
        assert_eq!(
            (reply.code, reply.stderr.as_str()),
            (128, NOT_A_REPOSITORY),
            "{line}"
        );
    }
}

#[test]
fn ac_973_a_scripted_error_exits_with_its_code_and_message() {
    let scenario =
        GitScenario::clean_on("main").failing(Call::Status, 128, "fatal: index broken\n");
    let reply = answer(&scenario, STATUS);
    assert_eq!(
        (reply.code, reply.stderr.as_str()),
        (128, "fatal: index broken\n")
    );
    assert_eq!(answer(&scenario, "rev-parse --show-prefix").code, 0);
}

#[test]
fn ac_969_ac_970_an_unscripted_call_names_each_argument_and_exits_2() {
    let reply = answer(&GitScenario::clean_on("main"), "log --oneline -1");
    assert_eq!(reply.code, UNSCRIPTED);
    assert_eq!(UNSCRIPTED, 2);
    assert!(
        reply.stderr.contains("git log --oneline -1"),
        "{}",
        reply.stderr
    );
}

#[test]
fn ac_968_the_executable_reads_its_scenario_from_the_environment() {
    let dir = std::env::temp_dir().join(format!("software-plant-git-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let installed = GitScenario::clean_on("trunk").install(&dir);
    let output = installed
        .apply(&mut Command::new("git"))
        .args(["symbolic-ref", "--quiet", "--short", "HEAD"])
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(output.stdout, b"trunk\n");
}

#[test]
fn the_executable_without_a_scenario_exits_2() {
    let output = Command::new(software_plant::git_executable())
        .env_remove(software_plant::git::SCENARIO_ENV)
        .args(["rev-parse", "--show-prefix"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(UNSCRIPTED));
}

#[test]
fn a_scenario_round_trips_through_its_file() {
    let dir = std::env::temp_dir().join(format!("software-plant-file-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let scenario = GitScenario::clean_on("main")
        .below("sub/")
        .renamed("a", "b")
        .failing(Call::ShowPrefix, 1, "x");
    let path = dir.join("scenario.json");
    scenario.write(&path);
    let read = GitScenario::read(&path);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(read.unwrap(), scenario);
}

/// A fresh directory for one test's plant, named by the test.
fn plant(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("software-plant-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Runs one call through the installed simulator, with `GIT_OPTIONAL_LOCKS` set as given, and
/// returns the locks the plant recorded.
fn locks_after(name: &str, call: &[&str], optional_locks: Option<&str>) -> Vec<String> {
    let dir = plant(name);
    let installed = GitScenario::clean_on("main").install(&dir);
    let mut command = Command::new("git");
    installed
        .apply(&mut command)
        .env_remove(OPTIONAL_LOCKS_ENV)
        .args(call);
    if let Some(value) = optional_locks {
        command.env(OPTIONAL_LOCKS_ENV, value);
    }
    assert!(command.status().unwrap().success());
    let locks = installed.locks();
    let _ = std::fs::remove_dir_all(&dir);
    locks
}

/// No lock recorded.
const NONE: [&str; 0] = [];

const STATUS_CALL: [&str; 6] = [
    "status",
    "--porcelain=v1",
    "-z",
    "--untracked-files=all",
    "--",
    "spec",
];

#[test]
fn ac_979_a_status_call_with_optional_locks_records_the_index_lock() {
    assert_eq!(locks_after("lock", &STATUS_CALL, None), ["index.lock"]);
    assert_eq!(
        locks_after("lock-1", &STATUS_CALL, Some("1")),
        ["index.lock"]
    );
}

#[test]
fn ac_980_a_status_call_with_optional_locks_set_to_0_records_no_lock() {
    assert_eq!(locks_after("no-lock", &STATUS_CALL, Some("0")), NONE);
}

#[test]
fn ac_981_no_other_call_records_a_lock() {
    let prefix = ["rev-parse", "--show-prefix"];
    let branch = ["symbolic-ref", "--quiet", "--short", "HEAD"];
    assert_eq!(locks_after("prefix", &prefix, None), NONE);
    assert_eq!(locks_after("branch", &branch, None), NONE);
}

#[test]
fn installing_a_scenario_starts_a_fresh_record_of_locks() {
    let dir = plant("fresh");
    let scenario = GitScenario::clean_on("main");
    let installed = scenario.install(&dir);
    installed
        .apply(&mut Command::new("git"))
        .env_remove(OPTIONAL_LOCKS_ENV)
        .args(STATUS_CALL)
        .status()
        .unwrap();
    assert_eq!(installed.locks(), ["index.lock"]);
    let again = scenario.install(&dir);
    let locks = again.locks();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(locks, NONE);
}

const DRIVER: &str = "merge.loft.driver";

#[test]
fn config_get_prints_the_value_of_a_name_the_scenario_sets() {
    let scenario =
        GitScenario::clean_on("main").with_config(DRIVER, "loft merge-driver %O %A %B %P");
    let reply = scenario.answer(&[
        "config".to_owned(),
        "--local".to_owned(),
        "--get".to_owned(),
        DRIVER.to_owned(),
    ]);
    assert_eq!(
        (reply.code, reply.stdout),
        (0, b"loft merge-driver %O %A %B %P\n".to_vec())
    );
}

#[test]
fn config_get_exits_1_and_prints_nothing_for_a_name_the_scenario_does_not_set() {
    let reply = answer(
        &GitScenario::clean_on("main"),
        "config --local --get merge.loft.name",
    );
    assert_eq!(
        (reply.code, reply.stdout, reply.stderr),
        (1, Vec::new(), String::new())
    );
}

#[test]
fn config_set_sets_the_name_in_the_scenario_the_executable_reads() {
    let dir = plant("config");
    let installed = GitScenario::clean_on("main").install(&dir);
    let set = installed
        .apply(&mut Command::new("git"))
        .args(["config", "--local", DRIVER, "loft merge-driver %O %A %B %P"])
        .output()
        .unwrap();
    let get = installed
        .apply(&mut Command::new("git"))
        .args(["config", "--local", "--get", DRIVER])
        .output()
        .unwrap();
    let scenario = GitScenario::read(&installed.scenario).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!((set.status.code(), set.stdout), (Some(0), Vec::new()));
    assert_eq!(get.stdout, b"loft merge-driver %O %A %B %P\n");
    assert_eq!(
        scenario,
        GitScenario::clean_on("main").with_config(DRIVER, "loft merge-driver %O %A %B %P")
    );
}

#[test]
fn outside_a_repository_a_config_call_exits_128_with_gits_message() {
    let outside = GitScenario::outside_repository();
    for line in ["config --local --get x", "config --local x y"] {
        let reply = answer(&outside, line);
        assert_eq!(
            (reply.code, reply.stderr.as_str()),
            (128, LOCAL_OUTSIDE_A_REPOSITORY),
            "{line}"
        );
    }
}

const MERGE_HEAD: &str = "rev-parse -q --verify MERGE_HEAD";
const REBASE_HEAD: &str = "rev-parse -q --verify REBASE_HEAD";
const SHA: &str = "b97feb1503e857271adfbc4604548baba9ff4d2b";

#[test]
fn the_commit_being_merged_or_replayed_is_answered_and_exits_1_when_none_is() {
    let clean = GitScenario::clean_on("main");
    for line in [MERGE_HEAD, REBASE_HEAD] {
        let reply = answer(&clean, line);
        assert_eq!((reply.code, reply.stdout), (1, Vec::new()), "{line}");
    }
    let merging = GitScenario::clean_on("main").merging(SHA);
    assert_eq!(
        answer(&merging, MERGE_HEAD).stdout,
        format!("{SHA}\n").into_bytes()
    );
    assert_eq!(answer(&merging, REBASE_HEAD).code, 1);
    let rebasing = GitScenario::clean_on("main")
        .detached()
        .rebasing(SHA, "refs/heads/main");
    assert_eq!(
        answer(&rebasing, REBASE_HEAD).stdout,
        format!("{SHA}\n").into_bytes()
    );
    assert_eq!(answer(&rebasing, MERGE_HEAD).code, 1);
}

#[test]
fn a_git_path_is_answered_under_the_git_directory_from_the_working_directory() {
    let line = "rev-parse --git-path rebase-merge/head-name";
    let top = GitScenario::clean_on("main");
    assert_eq!(answer(&top, line).stdout, b".git/rebase-merge/head-name\n");
    let below = top.below("sub/dir/");
    assert_eq!(
        answer(&below, line).stdout,
        b"../../.git/rebase-merge/head-name\n"
    );
}

#[test]
fn an_installed_rebase_writes_the_branch_it_rebases_where_its_git_path_names() {
    let dir = plant("rebase");
    let scenario = GitScenario::clean_on("main")
        .detached()
        .rebasing(SHA, "refs/heads/main");
    let installed = scenario.install(&dir);
    let output = installed
        .apply(&mut Command::new("git"))
        .args(["rev-parse", "--git-path", "rebase-merge/head-name"])
        .output()
        .unwrap();
    let named = String::from_utf8(output.stdout).unwrap();
    let held = std::fs::read_to_string(named.trim_end()).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(held, "refs/heads/main\n");
}

#[test]
fn unmerged_paths_are_answered_as_ls_files_prints_their_index_entries() {
    let scenario = GitScenario::clean_on("main")
        .merging(SHA)
        .unmerged(
            "100644",
            "20833b50d6fa1b4f9378bf129363513617191658",
            1,
            "loft.lock.json",
        )
        .unmerged(
            "100644",
            "7dc89e9dba4a0afa92e83f87c412b1aa11880f00",
            2,
            "loft.lock.json",
        );
    let reply = answer(&scenario, "ls-files -u -z --full-name -- :/");
    assert_eq!(
        reply.stdout,
        b"100644 20833b50d6fa1b4f9378bf129363513617191658 1\tloft.lock.json\0\
          100644 7dc89e9dba4a0afa92e83f87c412b1aa11880f00 2\tloft.lock.json\0"
    );
    let clean = answer(
        &GitScenario::clean_on("main"),
        "ls-files -u -z --full-name -- :/",
    );
    assert_eq!((clean.code, clean.stdout), (0, Vec::new()));
}

#[test]
fn the_lock_a_revision_holds_is_answered_or_refused_with_git_s_message() {
    let scenario = GitScenario::clean_on("main")
        .below("sub/")
        .merging(SHA)
        .lock_at("HEAD", "{\"a\": 1}\n");
    let held = answer(&scenario, "cat-file blob HEAD:./loft.lock.json");
    assert_eq!((held.code, held.stdout), (0, b"{\"a\": 1}\n".to_vec()));
    let missing = answer(&scenario, "cat-file blob MERGE_HEAD:./loft.lock.json");
    assert_eq!(
        (missing.code, missing.stderr.as_str()),
        (
            128,
            "fatal: path 'sub/loft.lock.json' does not exist in 'MERGE_HEAD'\n"
        )
    );
}

#[test]
fn outside_a_repository_the_calls_of_a_stopped_merge_exit_128() {
    let outside = GitScenario::outside_repository();
    for line in [
        MERGE_HEAD,
        "ls-files -u -z --full-name -- :/",
        "cat-file blob HEAD:./loft.lock.json",
    ] {
        let reply = answer(&outside, line);
        assert_eq!(
            (reply.code, reply.stderr.as_str()),
            (128, NOT_A_REPOSITORY),
            "{line}"
        );
    }
}

/// The shell alias with which `loft merge-driver --install` runs `<command> --version`.
fn launcher(command: &str) -> Vec<String> {
    vec![
        "-c".to_owned(),
        format!("alias.loft-launcher=!{command} --version"),
        "loft-launcher".to_owned(),
    ]
}

#[test]
fn the_launcher_alias_prints_the_version_the_scenario_gives_the_command() {
    let scenario = GitScenario::clean_on("main").launcher("npx --no-install loft", "loft 2.0.0\n");
    let reply = scenario.answer(&launcher("npx --no-install loft"));
    assert_eq!((reply.code, reply.stdout), (0, b"loft 2.0.0\n".to_vec()));
    let outside = GitScenario::outside_repository().launcher("loft", "loft 2.0.0\n");
    assert_eq!(outside.answer(&launcher("loft")).code, 0);
}

#[test]
fn the_launcher_alias_exits_127_for_a_command_the_scenario_does_not_give() {
    let reply = GitScenario::clean_on("main").answer(&launcher("loft"));
    assert_eq!((reply.code, reply.stdout), (127, Vec::new()));
}
