//! The `yard` skill, composed for Claude Code: one `SKILL.md` of at most 500 lines whose
//! frontmatter holds its name and description alone, which names the tools the Loft mod registers
//! and no other, and which teaches the habits that make Yard Log worth reading.

use std::collections::BTreeSet;
use std::fs;

use crate::common::{Scratch, stderr, workspace};

const SKILL: &str = "plugins/claude/loft/skills/yard";
const TOOLS: &str = "plugins/claude/loft/hooks/adapter/tools.ts";
const SOURCE: &str = "crates/loft-cli/skill/yard";

#[test]
fn the_shared_yard_workflow_names_no_host_or_native_tool() {
    let text = read(&format!("{SOURCE}/SKILL.md"));
    let lower = text.to_ascii_lowercase();
    for native in [
        "claude",
        "codex",
        "askuserquestion",
        "taskcreate",
        "taskupdate",
        "enterplanmode",
        "exitplanmode",
        "mcp__",
        "yard_",
    ] {
        assert!(!lower.contains(native), "shared source names {native}");
    }
    for habit in [
        "## Open Yard Log",
        "## Read before",
        "## Open a stage",
        "## Keep each task",
        "## Mark the fixes",
        "## Ask once",
        "## Write what each stage produced",
        "## Record each choice",
        "## Record Loft's reports",
        "## Have the spec change approved",
        "## Plan the build",
        "only the steps",
        "## Commit",
        "## Close it",
    ] {
        assert!(text.contains(habit), "shared source omits {habit}");
    }
}

#[test]
fn the_claude_yard_adaptation_holds_the_native_tools_dialogs_tasks_and_planning() {
    let text = read(&format!("{SOURCE}/agents/claude.md"));
    assert_eq!(tool_names(&text), tool_names(&read(TOOLS)));
    for native in [
        "AskUserQuestion",
        "TaskCreate",
        "TaskUpdate",
        "metadata: { type: \"fix\" }",
        "EnterPlanMode",
        "ExitPlanMode",
        "auto mode",
        "dialog",
    ] {
        assert!(text.contains(native), "Claude adaptation omits {native}");
    }
    assert!(
        !text.starts_with("---"),
        "only the common source has frontmatter"
    );
}

#[test]
fn the_committed_yard_package_equals_the_shared_source_with_its_claude_adaptation() {
    let common = read(&format!("{SOURCE}/SKILL.md"));
    let claude = read(&format!("{SOURCE}/agents/claude.md"));
    assert_eq!(
        read(&format!("{SKILL}/SKILL.md")),
        format!("{common}\n{claude}")
    );
}

#[test]
fn yard_is_packaged_for_each_adapter_but_not_installed_as_a_standalone_skill() {
    for host in ["claude", "codex"] {
        assert!(
            workspace()
                .join(format!("plugins/{host}/loft/skills/yard/SKILL.md"))
                .is_file()
        );
    }
    let scratch = Scratch::empty();
    let output = scratch.loft(&["skill", "install", "--no-plugin"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    for host in [".claude", ".agents"] {
        assert!(
            scratch
                .0
                .join(host)
                .join("skills/loft-spec/SKILL.md")
                .is_file()
        );
        assert!(!scratch.0.join(host).join("skills/yard").exists());
    }
}

fn read(path: &str) -> String {
    fs::read_to_string(workspace().join(path)).unwrap_or_else(|_| panic!("no {path}"))
}

/// The skill's text with every run of white space one space, so a phrase reads across lines.
fn skill() -> String {
    let text = read(&format!("{SKILL}/SKILL.md"));
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Holds the skill to each phrase.
fn says(phrases: &[&str]) {
    let text = skill();
    for phrase in phrases {
        assert!(
            text.contains(phrase),
            "the yard skill does not say: {phrase}"
        );
    }
}

/// The names `yard_<name>` in `text`.
fn tool_names(text: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut rest = text;
    while let Some(start) = rest.find("yard_") {
        let name: String = rest[start..]
            .chars()
            .take_while(|c| c.is_ascii_lowercase() || *c == '_')
            .collect();
        found.insert(name);
        rest = &rest[start + "yard_".len()..];
    }
    found
}

#[test]
fn the_yard_skill_is_one_file_of_at_most_500_lines_whose_frontmatter_holds_its_name_and_description_alone()
 {
    let files: Vec<String> = fs::read_dir(workspace().join(SKILL))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(files, ["SKILL.md"]);
    let text = read(&format!("{SKILL}/SKILL.md"));
    assert!(
        text.lines().count() <= 500,
        "{} lines",
        text.lines().count()
    );
    let frontmatter: Vec<&str> = text
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n"))
        .map(|(head, _)| head.lines().collect())
        .expect("a frontmatter between --- lines");
    let fields: Vec<&str> = frontmatter
        .iter()
        .map(|line| line.split_once(':').unwrap().0)
        .collect();
    assert_eq!(fields, ["name", "description"]);
    assert_eq!(frontmatter[0], "name: yard");
}

#[test]
fn the_yard_skill_names_each_tool_the_mod_registers_and_no_other() {
    let source = read(TOOLS);
    let declared = source
        .lines()
        .find_map(|line| line.strip_prefix("export type ToolName = "))
        .unwrap_or_else(|| panic!("{TOOLS} declares no ToolName"));
    let registered = tool_names(declared);
    assert_eq!(registered.len(), 6, "{registered:?}");
    let text = skill();
    assert_eq!(tool_names(&text), registered);
    for name in &registered {
        assert!(text.contains(&format!("`mcp__loft__{name}`")), "{name}");
    }
}

#[test]
fn the_yard_skill_tells_the_agent_to_open_yard_log_as_the_work_begins() {
    says(&[
        "Your first write on a branch opens the branch's Yard Log",
        "Open it when work on the branch begins, before the work's first step",
        "Yard Log, once closed, takes no more writes: more work goes on a branch of its own",
    ]);
}

#[test]
fn the_yard_skill_tells_the_agent_to_open_a_stage_for_each_step_and_end_it() {
    says(&[
        "Before each step of the work, write a stage for it, `active`, with a goal that names what the step leaves done",
        "mark the stage `done`, or `dropped`",
        "Each agent has its own active stage",
        "opens a stage of its own for its work",
    ]);
}

#[test]
fn the_yard_skill_tells_the_agent_to_build_only_once_the_person_asks_for_the_build() {
    says(&[
        "Begin no stage that builds the change unless the person asked for the build as well as the plan",
        "stop once the plan is written, and leave the build's stages `planned`",
        "Tell the person the plan is ready and that the build waits for them to ask for it",
        "A plan the person approved counts as asking for the build it describes",
        "a change to the specification the person approved does not count as asking for its build",
    ]);
}

#[test]
fn the_yard_skill_tells_the_agent_to_change_each_task_s_state_as_its_work_starts_and_ends() {
    says(&[
        "mark a task `active` in a write before its work starts",
        "As soon as its work ends, mark it `done`, or `dropped`",
        "before the next task is marked `active`",
        "Never mark the tasks of a stage `done` in one write at its end",
    ]);
}

#[test]
fn the_yard_skill_tells_the_agent_to_give_the_type_fix_to_a_task_that_corrects_work_at_the_person_s_request()
 {
    says(&[
        "A task is a fix where the person asked you to correct work already done",
        "A task you take on by yourself, a correction of your own included, is `regular`",
        "Create a fix with `TaskCreate` and `metadata: { type: \"fix\" }`",
        "write its `task` block with `type: fix`",
    ]);
}

#[test]
fn the_yard_skill_tells_the_agent_to_send_a_decision_point_s_questions_together_and_wait() {
    says(&[
        "send them together as one submission, rather than ask them one at a time",
        "Give each question a recommended option, and put the reasoning for each option in its `detail`",
        "`yard_wait` holds the turn until the person answers on the page or in the dialog",
        "Read the answers and outcomes from the wait's result before you go on",
    ]);
}

#[test]
fn the_yard_skill_tells_the_agent_to_write_what_each_stage_produced_in_markdown() {
    says(&[
        "When a stage ends, write a note of what it produced",
        "Write the note in Markdown, with a table where things are compared and a `mermaid` diagram",
    ]);
}

#[test]
fn the_yard_skill_tells_the_agent_to_have_the_person_approve_a_spec_change_before_it_is_committed()
{
    says(&[
        "send an `approval` of the change right after its `spec-change` block, in the same stage",
        "revise `spec/` without committing it, then record a new `spec-change` block and send a new approval right after it",
        "Commit `spec/` once the person approves the change, and not before",
    ]);
}

#[test]
fn the_yard_skill_tells_the_agent_to_plan_the_build_in_plan_mode_once_the_spec_change_is_approved()
{
    says(&[
        "Once the person has approved a change to `spec/` and it is committed, call `EnterPlanMode` before you write any code of the build",
        "present the build as a plan with `ExitPlanMode`, worked out from the specification the person approved",
        "The build begins only once the person approves that plan",
        "revise in plan mode with their reason",
    ]);
}

#[test]
fn the_yard_skill_tells_the_agent_to_plan_again_only_the_steps_a_later_requirement_change_touches()
{
    says(&[
        "commit the change to `spec/` and record its `spec-change` view before you write any code that meets it",
        "present with `ExitPlanMode` only the steps whose criteria the change touches",
        "The steps of the approved plan that the change leaves untouched stand, and are not presented again",
    ]);
}

#[test]
fn the_yard_skill_tells_the_agent_to_record_loft_s_views_rather_than_retell_them() {
    says(&[
        "Record a `spec-change` view once a change to `spec/` passes `loft fmt --check` and `loft check --fail-on warning`, whether or not it is committed",
        "Record a `decision` view once a decision of the specification is accepted",
        "Record a `findings` view after `loft check`, with a reason for each finding left in place",
        "Record a `trace` view once the tests are linked and the links accepted",
        "Record the publish with `yard_published` on the base branch once `loft publish` is committed",
        "name the branch whose Yard Log the merge brought in",
    ]);
}

#[test]
fn the_yard_skill_tells_the_agent_to_record_each_choice_as_a_decision_linked_to_its_question() {
    says(&[
        "Each time the person makes a choice, by an answer or a decision, write a `decision` block",
        "the question it settles as its `question`",
    ]);
}

#[test]
fn the_yard_skill_tells_the_agent_to_commit_the_yard_folder_with_the_work_unless_git_ignores_it() {
    says(&[
        "Commit `.yard/` in each commit of the work",
        "Answers the person gave since the last commit are committed with the next",
        "its token totals under `.yard/<branch>/tokens/` and its activity under `.yard/<branch>/activity/`",
        "Where `yard_read` reports that git ignores `.yard/`",
        "commit nothing of `.yard/`",
    ]);
}

#[test]
fn the_yard_skill_tells_the_agent_to_close_and_commit_yard_log_before_the_merge() {
    says(&[
        "close Yard Log with `yard_close`, commit the closed Yard Log unless git ignores `.yard/`, and only then merge",
    ]);
}

#[test]
fn the_yard_skill_tells_the_agent_to_read_what_changed_and_base_each_write_on_it() {
    says(&[
        "When a session starts, and each time a wait returns, read Yard Log with `since` set to the revision you last read",
        "Base each write on the revision you last read, in `basedOn`",
        "names the blocks that changed under you: read them, then write again",
    ]);
}

#[test]
fn the_yard_skill_tells_the_agent_that_a_read_gives_each_report_by_its_size_and_reports_gives_them()
{
    says(&[
        "A read gives each `loft` block's report by its size in `reportBytes`, not the report",
        "read with `reports` as well as `since`",
    ]);
}

#[test]
fn the_yard_skill_tells_the_agent_to_check_a_report_by_the_counts_a_read_gives() {
    says(&[
        "the number of items of each list it counts in `reportCounts`",
        "check what a report holds by its counts",
    ]);
}
