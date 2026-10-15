//! `OpenSpec` projects written for the converter's tests, and the conversion of them.

use loftspec::convert::openspec::{Conversion, ConvertError, Project, convert};
use loftspec::identity::Key;
use loftspec::ids::DocumentId;
use loftspec::tree::{Feature, Requirement};

/// The document key the tests convert under.
pub const DOCUMENT: &str = "01a13d20-0000-7000-8000-000000000001";

pub fn document() -> DocumentId {
    DocumentId::new(Key::parse(DOCUMENT).unwrap())
}

/// A main spec as `OpenSpec`'s templates write it: a title, a Purpose and two requirements with
/// scenarios.
pub const AUTH: &str = "# auth Specification

## Purpose
Users sign in with a password and sign out.

## Requirements

### Requirement: Sign in
The system SHALL let a user sign in with a password.

A locked account cannot sign in.

#### Scenario: Valid credentials
- **WHEN** a user submits the right password
- **THEN** the user is signed in

#### Scenario: Wrong password
- **WHEN** a user submits a wrong password
- **THEN** the system MUST refuse the sign-in

### Requirement: Sign out
When a user signs out, the system SHALL end the session.

#### Scenario: Session ends
- **WHEN** the user signs out
- **THEN** the session ends
";

/// A project of the given main specs, by their paths below `specs/`.
pub fn project(specs: &[(&str, &str)]) -> Project {
    Project {
        specs: specs
            .iter()
            .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
            .collect(),
        ..Project::default()
    }
}

/// A project of one main spec, `auth`.
pub fn one(text: &str) -> Project {
    project(&[("auth/spec.md", text)])
}

/// Converts `project` under the test document key into the Product `Shop`.
pub fn converted(project: &Project) -> Conversion {
    try_convert(project).expect("the project converts")
}

pub fn try_convert(project: &Project) -> Result<Conversion, ConvertError> {
    convert(project, document(), "Shop")
}

/// The Features of the one Product, in order.
pub fn features(conversion: &Conversion) -> Vec<&Feature> {
    conversion.document.products[0]
        .feature_groups
        .iter()
        .flat_map(|group| &group.features)
        .collect()
}

/// The first Feature's Requirements.
pub fn requirements(conversion: &Conversion) -> &[Requirement] {
    &conversion.document.products[0].feature_groups[0].features[0].requirements
}
