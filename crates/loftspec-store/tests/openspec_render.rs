//! The `OpenSpec` projection of an imported fixture project: each main spec renders back to the
//! bytes it was read from.

use std::collections::BTreeMap;
use std::path::Path;

use loftspec::convert::openspec::convert;
use loftspec::ids::DocumentId;
use loftspec::render::openspec::openspec;
use loftspec_store::openspec::read_project;

#[test]
fn each_fixture_projects_main_specs_render_back_byte_for_byte() {
    for fixture in ["faithful", "quiet"] {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata/openspec")
            .join(fixture)
            .join("openspec");
        let project = read_project(&dir).unwrap();
        let conversion = convert(&project, DocumentId::mint(), fixture).unwrap();
        let rendered = openspec(&conversion.document).unwrap().files;
        let read: BTreeMap<String, String> = project
            .specs
            .into_iter()
            .map(|(path, text)| (format!("specs/{path}"), text))
            .collect();
        assert_eq!(rendered, read, "{fixture}");
    }
}
