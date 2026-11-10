//! What the page server tells a mod of the trees it serves: the real path of each, in its answer
//! to a session heard and to a tree registered, and the top of a tree registered as the request
//! spelled it.

use serde_json::json;

use crate::common::Scratch;
use crate::served::Served;
use crate::yard::Yard;

#[test]
fn the_answer_to_a_session_heard_names_the_real_path_of_each_tree_served() {
    let yard = Yard::new();
    let real = yard.scratch.0.canonicalize().unwrap().display().to_string();
    let served = Served::start(&yard, &["--tree", &yard.scratch.0.display().to_string()]);
    let reply = served
        .post_as_mod("/api/heard", &json!({ "session": "s7" }))
        .json();
    assert_eq!(reply["trees"], json!([real]), "{reply}");
}

#[cfg(unix)]
#[test]
fn the_answer_to_a_tree_registered_names_its_top_by_its_real_path_and_as_spelled() {
    let yard = Yard::new();
    let served = Served::start(&yard, &[]);
    // A link to the tree: the request spells it one way, git names its real path.
    let links = Scratch::empty();
    let link = links.0.join("link");
    std::os::unix::fs::symlink(&yard.scratch.0, &link).unwrap();
    let real = yard.scratch.0.canonicalize().unwrap().display().to_string();
    let spelled = link.display().to_string();
    let reply = served
        .post_as_mod("/api/register", &json!({ "tree": spelled }))
        .json();
    assert_eq!(reply["tree"], real.as_str(), "{reply}");
    assert_eq!(reply["spelled"], spelled.as_str(), "{reply}");
    assert_eq!(reply["trees"], json!([real]), "{reply}");
    let again = served
        .post_as_mod("/api/register", &json!({ "tree": real }))
        .json();
    assert!(again.get("spelled").is_none(), "{again}");
}
