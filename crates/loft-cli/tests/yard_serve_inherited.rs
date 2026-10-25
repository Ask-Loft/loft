//! What the page server lists of the Yard Logs carried from other branches: their count in the
//! list of Yard Logs, and a page of them at a time, once for each repository; how it reads each
//! Yard Log for the list again only once its root changed; and how a change to one reaches open
//! pages.

use std::fs;
use std::io::{BufReader, Write};
use std::net::TcpStream;
use std::time::Duration;

use serde_json::{Value, json};
use software_plant::GitScenario;

use crate::served::Served;
use crate::yard::BRANCH;
use crate::yard_serve_logs::{stream_until, tree, with_inherited};

#[test]
fn each_tree_lists_only_its_current_branch_s_yard_log_and_its_repository_counts_the_inherited_ones()
{
    let yard = with_inherited();
    let served = Served::start(&yard, &["--tree", &yard.scratch.0.display().to_string()]);
    let listing = served.get("/api/trees").json();
    let repository = &listing["repositories"][0];
    let logs = repository["trees"][0]["logs"].as_array().unwrap();
    assert_eq!(logs.len(), 1, "{listing}");
    assert_eq!(logs[0]["branch"], BRANCH);
    assert_eq!(logs[0]["inherited"], false);
    assert_eq!(logs[0]["waiting"], 1);
    assert_eq!(repository["inherited"], 1);
    assert!(
        !listing.to_string().contains("\"branch\":\"main\""),
        "the listing carries the count of the inherited Yard Logs, not the Yard Logs: {listing}"
    );
    assert_eq!(listing["count"], 1, "an inherited Yard Log waits on nobody");
    let waits = listing["waiting"].as_array().unwrap();
    assert_eq!(waits.len(), 1);
    assert_eq!(waits[0]["branch"], BRANCH);
    assert_eq!(waits[0]["blocks"], json!(["q1"]));
}

/// The page of `served`'s inherited Yard Logs of its only repository that `query` asks for.
fn inherited(served: &Served, query: &str) -> Value {
    let listing = served.get("/api/trees").json();
    let repository = listing["repositories"][0]["repository"]
        .as_str()
        .unwrap()
        .to_owned();
    served
        .get(&format!("/api/inherited?repository={repository}&{query}"))
        .json()
}

#[test]
fn a_repository_s_inherited_yard_logs_are_given_a_page_at_a_time_filtered_by_branch_or_title() {
    let yard = with_inherited();
    let served = Served::start(&yard, &["--tree", &yard.scratch.0.display().to_string()]);
    let all = inherited(&served, "");
    assert_eq!(all["total"], 1, "{all}");
    let main = &all["logs"][0];
    assert_eq!(main["branch"], "main");
    assert_eq!(main["inherited"], true);
    assert_eq!(main["tree"], tree(&yard.scratch));
    assert!(main["openedAt"].is_string(), "{main}");
    assert_eq!(inherited(&served, "filter=MAI")["total"], 1);
    assert_eq!(inherited(&served, "filter=shelf")["total"], 0);
    let past = inherited(&served, "from=1");
    assert_eq!(
        (
            past["total"].as_u64(),
            past["logs"].as_array().map(Vec::len)
        ),
        (Some(1), Some(0))
    );
}

#[test]
fn a_change_to_an_inherited_yard_log_reaches_open_pages_within_six_seconds() {
    let mut yard = with_inherited();
    let served = Served::start(&yard, &["--tree", &yard.scratch.0.display().to_string()]);
    let mut stream = TcpStream::connect(("127.0.0.1", served.port)).unwrap();
    let request = format!(
        "GET /api/stream HTTP/1.1\r\nhost: 127.0.0.1:{}\r\n\r\n",
        served.port
    );
    stream.write_all(request.as_bytes()).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(6)))
        .unwrap();
    let mut reader = BufReader::new(stream);
    assert!(stream_until(&mut reader, "hello"));
    std::thread::sleep(Duration::from_millis(600));
    // Written on `main`, whose Yard Log the page server's tree holds as inherited.
    yard.git = GitScenario::clean_on("main");
    let note = r#"[{"id": "n1", "kind": "note", "stage": "shape", "markdown": "Seen."}]"#;
    yard.write("s1", note, &[]);
    assert!(
        stream_until(&mut reader, "change"),
        "no change within 6 seconds"
    );
}

#[test]
fn an_entry_of_the_list_is_read_again_only_once_its_yard_log_s_root_changes() {
    let yard = with_inherited();
    let served = Served::start(&yard, &["--tree", &yard.scratch.0.display().to_string()]);
    let main = |page: Value| page["logs"][0].clone();
    let first = main(inherited(&served, ""));
    assert_eq!(first["branch"], "main", "{first}");
    assert!(first.get("error").is_none(), "{first}");
    // The root, unreadable now, keeps its size and its time: the entry is the one last read.
    let root = yard.folder("main").join("yardlog.yaml");
    let meta = fs::metadata(&root).unwrap();
    let time = meta.modified().unwrap();
    fs::write(&root, vec![b'x'; usize::try_from(meta.len()).unwrap()]).unwrap();
    let file = fs::File::options().write(true).open(&root).unwrap();
    file.set_modified(time).unwrap();
    assert_eq!(main(inherited(&served, "")), first);
    file.set_modified(time + Duration::from_secs(5)).unwrap();
    let again = main(inherited(&served, ""));
    assert!(again.get("error").is_some(), "{again}");
}
