//! The main specs, as read, turned into one Product with its Feature Groups, Features,
//! Requirements, Criteria and Sections, the provenance of each node and the findings.

use std::collections::{BTreeMap, BTreeSet};

use super::findings::{Finding, Rule};
use super::keys::{Identity, Keys, Kind};
use super::read::{Child, Part, Requirement as SourceRequirement, Source, Spec};
use super::sidecar::{OpenspecSidecar, Provenance, SourceFile};
use super::statement;
use crate::canonical::digest_bytes;
use crate::document::Document;
use crate::ext::Ext;
use crate::identity::Key;
use crate::ids::{
    CriterionId, DocumentId, FeatureGroupId, FeatureId, ProductId, RequirementId, SectionId,
};
use crate::slug::Slug;
use crate::tree::{Criterion, Details, Feature, FeatureGroup, Product, Requirement, Section};

/// The prefix of the Purpose `OpenSpec`'s archive command writes for a new capability.
const PLACEHOLDER: &str = "TBD - created by archiving change";

/// The Feature Group of the capabilities whose ids have one segment.
const FLAT_GROUP: &str = "Capabilities";

/// One conversion in progress.
pub(super) struct Build {
    document: DocumentId,
    keys: Keys,
    findings: Vec<Finding>,
    sidecar: OpenspecSidecar,
    rewritten: usize,
}

/// One capability: its id, the path of its file in the `openspec/` directory, and its text.
pub(super) struct Capability<'a> {
    pub id: String,
    pub file: String,
    pub text: &'a str,
}

impl Build {
    pub fn new(document: DocumentId) -> Self {
        Self {
            document,
            keys: Keys::new(document),
            findings: Vec::new(),
            sidecar: OpenspecSidecar::new(),
            rewritten: 0,
        }
    }

    /// The sidecar and the findings: first the count of rewritten statements, if any, then
    /// the findings in the order the build met them.
    pub fn finish(self) -> (OpenspecSidecar, Vec<Finding>) {
        let mut findings = Vec::new();
        if self.rewritten > 0 {
            let detail = self.rewritten.to_string();
            findings.push(Finding::about(Rule::ModalRewritten, detail));
        }
        findings.extend(self.findings);
        (self.sidecar, findings)
    }

    /// The document of one Product named `name`, holding `capabilities` in order.
    pub fn document(
        &mut self,
        name: &str,
        project_md: Option<&str>,
        capabilities: &[Capability<'_>],
    ) -> Document {
        let (key, product) = self.node(Kind::Product, Vec::new(), None, Some(name));
        let mut details = Details::default();
        if let Some(text) = project_md {
            let source = Source::new(text);
            self.source("project.md", text);
            let body = source.verbatim(source.whole());
            let parts = section_parts(&product, "Project");
            let at = Some(("project.md", 1));
            let (key, _) = self.node(Kind::Section, parts, at, Some("Project"));
            details.sections.push(Section {
                id: Some(SectionId::new(key)),
                name: "Project".to_owned(),
                body: body.unwrap_or_default(),
                sections: Vec::new(),
                ext: Ext::new(),
            });
        }
        let mut groups: Vec<FeatureGroup> = Vec::new();
        let mut by_segment: BTreeMap<Option<String>, usize> = BTreeMap::new();
        for capability in capabilities {
            let (segment, rest) = match capability.id.split_once('/') {
                Some((first, rest)) => (Some(first.to_owned()), rest.to_owned()),
                None => (None, capability.id.clone()),
            };
            let index = *by_segment.entry(segment.clone()).or_insert_with(|| {
                groups.push(self.group(segment.as_deref()));
                groups.len() - 1
            });
            let feature = self.feature(capability, &rest);
            groups[index].features.push(feature);
        }
        for group in &mut groups {
            Slug::make_unique(group.features.iter_mut().map(|feature| &mut feature.slug));
        }
        Slug::make_unique(groups.iter_mut().map(|group| &mut group.slug));
        let mut document = Document::empty(self.document);
        document.products.push(Product {
            id: Some(ProductId::new(key)),
            slug: Slug::derive(name, "product"),
            name: name.to_owned(),
            details,
            feature_groups: groups,
            constraints: Vec::new(),
        });
        document
    }

    fn group(&mut self, segment: Option<&str>) -> FeatureGroup {
        let name = segment.map_or_else(|| FLAT_GROUP.to_owned(), humanise);
        let parts = vec![segment.unwrap_or_default().to_owned()];
        let (key, _) = self.node(
            Kind::FeatureGroup,
            parts,
            None,
            segment.or(Some(FLAT_GROUP)),
        );
        FeatureGroup {
            id: Some(FeatureGroupId::new(key)),
            slug: Slug::derive(segment.unwrap_or(FLAT_GROUP), "feature-group"),
            name,
            details: Details::default(),
            features: Vec::new(),
            constraints: Vec::new(),
        }
    }

    fn feature(&mut self, capability: &Capability<'_>, rest: &str) -> Feature {
        let file = capability.file.as_str();
        self.source(file, capability.text);
        let source = Source::new(capability.text);
        if let Some(line) = source.front_matter {
            self.findings
                .push(Finding::at(Rule::FrontMatter, file.to_owned(), line, None));
        }
        let spec = source.read();
        let (key, identity) = self.keys.derive(Kind::Feature, vec![capability.id.clone()]);
        let title = spec.title.as_ref();
        self.provenance(
            key,
            Kind::Feature,
            Some((file, title.map_or(1, |(_, line)| *line))),
            Some(&capability.id),
            title.map(|(text, _)| text.as_str()),
        );
        let body = self.purpose(&spec, &source, file, key);
        let sections = self.feature_sections(&spec, &source, file, &identity);
        let mut names = BTreeSet::new();
        let requirements = spec
            .requirements
            .iter()
            .map(|requirement| {
                let repeated = !names.insert(requirement.name.clone());
                self.requirement(requirement, &source, file, &identity, repeated)
            })
            .collect();
        Feature {
            id: Some(FeatureId::new(key)),
            slug: Slug::derive(rest, "feature"),
            name: feature_name(title.map(|(text, _)| text.as_str()), &capability.id),
            details: Details {
                body,
                tags: Vec::new(),
                sections,
                ext: Ext::new(),
            },
            requirements,
            quality_attributes: Vec::new(),
        }
    }

    fn purpose(&mut self, spec: &Spec, source: &Source, file: &str, key: Key) -> Option<String> {
        let purpose = spec.purpose.as_ref()?;
        let text = source.verbatim(purpose.text.clone())?;
        if text.trim_start().starts_with(PLACEHOLDER) {
            let finding = Finding::at(
                Rule::PlaceholderPurpose,
                file.to_owned(),
                purpose.line,
                Some(key),
            );
            self.findings.push(finding);
            return None;
        }
        Some(text)
    }

    /// The Feature's Sections in source order: the introduction, the other level-2 sections
    /// and the text of `Requirements` outside every requirement.
    fn feature_sections(
        &mut self,
        spec: &Spec,
        source: &Source,
        file: &str,
        feature: &Identity,
    ) -> Vec<Section> {
        let mut parts: Vec<(Rule, &Part)> = Vec::new();
        parts.extend(
            spec.introduction
                .iter()
                .map(|part| (Rule::ExtraSection, part)),
        );
        parts.extend(spec.sections.iter().map(|(part, delta)| {
            let rule = if *delta {
                Rule::DeltaSection
            } else {
                Rule::ExtraSection
            };
            (rule, part)
        }));
        parts.extend(
            spec.stray
                .iter()
                .map(|part| (Rule::StrayRequirementsText, part)),
        );
        parts.sort_by_key(|(_, part)| part.line);
        parts
            .into_iter()
            .map(|(rule, part)| self.reported_section(rule, part, feature, source, file))
            .collect()
    }

    /// A Section, with a finding of `rule` at its heading.
    fn reported_section(
        &mut self,
        rule: Rule,
        part: &Part,
        parent: &Identity,
        source: &Source,
        file: &str,
    ) -> Section {
        let section = self.section(part, parent, source, file);
        let key = section.id.map(SectionId::key);
        self.findings
            .push(Finding::at(rule, file.to_owned(), part.line, key));
        section
    }

    fn section(&mut self, part: &Part, parent: &Identity, source: &Source, file: &str) -> Section {
        let parts = section_parts(parent, &part.name);
        let at = Some((file, part.line));
        let (key, identity) = self.node(Kind::Section, parts, at, Some(&part.name));
        let body = source.verbatim(part.text.clone()).unwrap_or_default();
        let sections = part
            .children
            .iter()
            .map(|child| self.section(child, &identity, source, file))
            .collect();
        Section {
            id: Some(SectionId::new(key)),
            name: part.name.clone(),
            body,
            sections,
            ext: Ext::new(),
        }
    }

    fn requirement(
        &mut self,
        requirement: &SourceRequirement,
        source: &Source,
        file: &str,
        feature: &Identity,
        repeated: bool,
    ) -> Requirement {
        let mut parts = feature.parts.clone();
        parts.push(requirement.name.clone());
        let at = Some((file, requirement.line));
        let (key, identity) = self.node(Kind::Requirement, parts, at, Some(&requirement.name));
        let split = statement::split(source, requirement.text.clone(), &requirement.name);
        self.rewritten += usize::from(split.rewritten);
        let at = |rule, line| Finding::at(rule, file.to_owned(), line, Some(key));
        if repeated {
            self.findings
                .push(at(Rule::DuplicateRequirement, requirement.line));
        }
        if !split.modal {
            self.findings.push(at(Rule::NoModal, requirement.line));
        }
        for line in &split.metadata {
            self.findings.push(at(Rule::Metadata, *line));
        }
        let mut criteria = Vec::new();
        let mut sections = Vec::new();
        for child in &requirement.children {
            if let Some(name) = &child.scenario {
                criteria.push(self.criterion(child, name, &identity, source, file));
            } else {
                let rule = Rule::NonScenarioChild;
                sections.push(self.reported_section(rule, &child.part, &identity, source, file));
            }
        }
        Requirement {
            id: Some(RequirementId::new(key)),
            statement: split.statement,
            details: Details {
                body: split.body,
                tags: Vec::new(),
                sections,
                ext: Ext::new(),
            },
            criteria,
        }
    }

    fn criterion(
        &mut self,
        child: &Child,
        name: &str,
        requirement: &Identity,
        source: &Source,
        file: &str,
    ) -> Criterion {
        let name = if name.is_empty() { "Scenario" } else { name };
        let mut parts = requirement.parts.clone();
        parts.push(name.to_owned());
        let at = Some((file, child.part.line));
        let (key, _) = self.node(Kind::Criterion, parts, at, Some(name));
        Criterion {
            id: Some(CriterionId::new(key)),
            name: name.to_owned(),
            details: Details {
                body: source.verbatim(child.part.text.clone()),
                tags: Vec::new(),
                sections: Vec::new(),
                ext: Ext::new(),
            },
        }
    }

    fn source(&mut self, path: &str, text: &str) {
        self.sidecar.sources.push(SourceFile {
            path: path.to_owned(),
            sha256: digest_bytes(text.as_bytes()).to_string(),
        });
    }

    /// The key of a node of `kind` whose source identity is `parts`, and its identity, with
    /// where it came from recorded: its file and line where it has them, and its name.
    fn node(
        &mut self,
        kind: Kind,
        parts: Vec<String>,
        at: Option<(&str, usize)>,
        name: Option<&str>,
    ) -> (Key, Identity) {
        let (key, identity) = self.keys.derive(kind, parts);
        self.provenance(key, kind, at, None, name);
        (key, identity)
    }

    /// Records where a node came from: its file and the line of its heading, where it has
    /// them, its capability and the name the source gives it.
    fn provenance(
        &mut self,
        key: Key,
        kind: Kind,
        at: Option<(&str, usize)>,
        capability: Option<&str>,
        name: Option<&str>,
    ) {
        self.sidecar.nodes.push(Provenance {
            key,
            kind: kind.name(),
            path: at.map(|(path, _)| path.to_owned()),
            line: at.map(|(_, line)| line),
            capability: capability.map(str::to_owned),
            name: name.map(str::to_owned),
        });
    }
}

/// A Section's source identity: its parent's kind and source identity, then its heading.
fn section_parts(parent: &Identity, heading: &str) -> Vec<String> {
    let mut parts = vec![parent.kind.name().to_owned()];
    parts.extend(parent.parts.iter().cloned());
    parts.push(heading.to_owned());
    parts
}

/// The Feature's name: the level-1 heading without a trailing ` Specification`, or the last
/// segment of the capability id made readable when there is no heading or it is the id.
fn feature_name(title: Option<&str>, id: &str) -> String {
    let named = title.map(|title| title.strip_suffix(" Specification").unwrap_or(title).trim());
    match named {
        Some(name) if !name.is_empty() && name != id => name.to_owned(),
        _ => humanise(id.rsplit('/').next().unwrap_or(id)),
    }
}

/// A path segment read as a name: each hyphen and underscore a space, the first letter in
/// upper case.
fn humanise(segment: &str) -> String {
    let spaced = segment.replace(['-', '_'], " ");
    let mut characters = spaced.chars();
    characters.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(characters).collect()
    })
}
