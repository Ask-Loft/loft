//! The `loft` command line (FG-9, FG-10). Exit codes: 0 on success, 1 on an error or a
//! failed check, 2 on a usage error (RQ-154). Every line is written through `out`, which a
//! closed standard output does not panic.
#![deny(clippy::print_stdout, clippy::print_stderr)]

mod base;
mod check;
mod cli;
mod endpoints;
mod environment;
mod find;
mod finish;
mod fmt;
mod import;
mod import_openspec;
mod init;
mod install;
mod json;
mod locate;
mod merge_driver;
mod network;
mod opened;
mod out;
mod publish;
mod reconcile;
mod record_install;
mod render;
mod review;
mod settings;
mod skill;
mod status;
mod stop;
mod switch_command;
mod switches;
mod telemetry;
mod terms;
mod update_check;
mod verify;
mod version;
mod yard;

use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;

use clap::{CommandFactory, Parser};
use loftspec::lint::Severity;
use loftspec_store::merge_driver::LAUNCHER;

use cli::{Cli, Command, Format, ImportSource, MergeDriverArgs, SkillAction};
use finish::Finished;

fn main() -> ExitCode {
    let code = run(Cli::parse());
    out::finish(code)
}

/// Runs the command the arguments name and gives the code it exits with.
fn run(cli: Cli) -> ExitCode {
    if cli.version {
        version::run(cli.format == Some(Format::Json));
        return ExitCode::SUCCESS;
    }
    let Some(command) = cli.command else {
        // Clap prints the usage for a bare `loft`; any other way to reach here is a usage error.
        let _ = Cli::command().print_help();
        return ExitCode::from(2);
    };
    let started = Instant::now();
    let sends = command.event_name();
    let finished = dispatch(command);
    let root = Path::new(".");
    let mut jobs = Vec::new();
    if let Some(name) = sends {
        jobs.extend(telemetry::prepare(name, &finished, started, root));
        jobs.extend(update_check::after_run());
    }
    network::run(jobs);
    finished.code
}

/// Runs `loft import` from `source` in `root`.
fn import(root: &Path, source: ImportSource) -> Finished {
    match source {
        ImportSource::V1 {
            export,
            counters,
            output,
        } => import::run(root, &export, counters.as_deref(), output.json()),
        ImportSource::Openspec {
            dir,
            product,
            output,
        } => import_openspec::run(root, &dir, product.as_deref(), output.json()),
    }
}

/// Runs `loft merge-driver` with `args` in `root`.
fn merge_driver(root: &Path, args: MergeDriverArgs) -> Finished {
    match args {
        MergeDriverArgs {
            install: true,
            command,
            output,
            ..
        } => merge_driver::install(root, command.as_deref().unwrap_or(LAUNCHER), output.json()),
        MergeDriverArgs {
            base: Some(base),
            ours: Some(ours),
            theirs: Some(theirs),
            path: Some(path),
            output,
            ..
        } => merge_driver::run(
            root,
            &merge_driver::Inputs {
                base: &base,
                ours: &ours,
                theirs: &theirs,
                path: &path,
            },
            output.json(),
        ),
        // Clap requires the four paths unless `--install` is given.
        MergeDriverArgs { .. } => Finished::usage(),
    }
}

/// Whether writing the skill enables the plugin, given `--no-plugin`.
const fn plugin(no_plugin: bool) -> skill::Plugin {
    if no_plugin {
        skill::Plugin::Leave
    } else {
        skill::Plugin::Enable
    }
}

/// Runs `command` in the current directory.
fn dispatch(command: Command) -> Finished {
    let root = Path::new(".");
    match command {
        Command::Init {
            no_skill,
            no_plugin,
            output,
        } => {
            let skill = (!no_skill).then_some(plugin(no_plugin));
            init::run(root, skill, output.json())
        }
        Command::Fmt { check, output } => fmt::run(root, check, output.json()),
        Command::Check {
            fail_on,
            baseline,
            output,
        } => {
            let options = check::Options {
                fail_on: fail_on.map(Severity::from),
                baseline: baseline.as_deref(),
            };
            check::run(root, &options, output.json())
        }
        Command::Import { source } => import(root, source),
        Command::Publish { output } => publish::run(root, output.json()),
        Command::Find { query, output } => find::run(root, &query, output.json()),
        Command::Terms { output } => terms::run(root, output.json()),
        Command::Reconcile {
            fail_on,
            accept,
            accept_stale,
            output,
        } => {
            let options = reconcile::Options {
                fail_on: fail_on.map(Severity::from),
                accept: &accept,
                accept_stale,
            };
            reconcile::run(root, &options, output.json())
        }
        Command::Verify { output } => verify::run(root, output.json()),
        Command::Review(args) => review::run(root, &args.options(), args.output.json()),
        Command::Status { base, output } => status::run(root, base.as_deref(), output.json()),
        Command::MergeDriver(args) => merge_driver(root, args),
        Command::Render {
            markdown,
            json,
            openspec,
            force,
            output,
        } => render::run(
            root,
            render::Targets {
                markdown,
                json,
                openspec,
                force,
            },
            output.json(),
        ),
        Command::Skill {
            action:
                SkillAction::Install {
                    host,
                    no_plugin,
                    output,
                },
        } => skill::run(root, host, plugin(no_plugin), output.json()),
        Command::RecordInstall { way, output } => record_install::run(way, output.json()),
        Command::Telemetry { action } => {
            let (action, as_json) = action.split();
            switch_command::telemetry(action, as_json)
        }
        Command::UpdateCheck { action } => {
            let (action, as_json) = action.split();
            switch_command::update_check(action, as_json)
        }
        Command::Yard { action } => yard::run(root, action),
    }
}
