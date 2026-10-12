# Loft Specification Format

> Loft `D-11` · Authored State `a0654aa3-9372-4a39-bba5-dffa80ef625f` · content `f581dfb3603d7286b799d28b3e6c611a95c8310ad1845f2e21fa963e745e4195`

## Capability Model

### P-1 · Loft Specification Format

The portable document model and the rules every implementation must follow so that two implementations agree on a document's content root. Obligations fall on roles — the reader, which turns a `spec/` tree into a document; the writer, which turns a document into a `spec/` tree; the formatter, the writer in canonical layout; the hasher; the converter from a Loft v1 export; the validator; the linter; the renderer; the publisher; and the merge driver. A role names an obligation, not a module: one program may fill every role.

**FT-1 · Facet Tag · portable**

#### FG-1 · Identity and addressing

Keys, labels, aliases and bindings; the content view and the hash of a node; the roots; the manifest, the lock and the derived-from record.

**FT-2 · Facet Tag · identity**

**FT-3 · Facet Tag · portable**

##### F-1 · Keys

The UUID key that addresses a node: written on each node, kept through each edit, minted by the formatter when absent, and checked by the reader for presence and uniqueness.

###### RQ-1 · The writer shall write a key on every node.

**AC-1 · Acceptance Criterion · Every node record in a written file carries a `key` field.**

**AC-2 · Acceptance Criterion · Every `key` value is an RFC 9562 UUID in lowercase hyphenated form.**

**AC-3 · Acceptance Criterion · A key read from a file is written back unchanged.**

**AC-4 · Acceptance Criterion · A key of any UUID version is written back unchanged.**

###### RQ-2 · The writer shall keep a node's key through every edit of the node.

**AC-5 · Acceptance Criterion · A node's key is unchanged after its name changes.**

**AC-6 · Acceptance Criterion · A node's key is unchanged after its body changes.**

**AC-7 · Acceptance Criterion · A node's key is unchanged after the node moves to another parent.**

**AC-8 · Acceptance Criterion · A node's key is unchanged after the node is relabelled.**

**AC-249 · Acceptance Criterion · A node's key is unchanged after its kind changes.**

**AC-250 · Acceptance Criterion · A node's key is unchanged after its tags change.**

###### RQ-3 · When formatting a node without a key, the formatter shall mint a version 7 UUID as its key.

**AC-9 · Acceptance Criterion · A node without a `key` field has one after formatting.**

**AC-10 · Acceptance Criterion · A minted key is a version 7 UUID.**

**AC-11 · Acceptance Criterion · Two keys minted in one run differ.**

**AC-12 · Acceptance Criterion · A node that already has a key keeps it after formatting.**

###### RQ-4 · If two keyed records of one document carry the same key, then the reader shall report an error naming both files.

**AC-13 · Acceptance Criterion · The error names both file paths.**

**AC-14 · Acceptance Criterion · The error names the duplicated key.**

**AC-15 · Acceptance Criterion · No document is produced from a tree with a duplicate key.**

**AC-251 · Acceptance Criterion · When both nodes are in one file, the error names that file once with the line and column of each node.**

**AC-848 · Acceptance Criterion · A node and a decision that carry the same key are an error.**

**AC-849 · Acceptance Criterion · Two registry entries that carry the same key are an error.**

###### RQ-5 · If a keyed record has no key, then the reader shall report an error naming the file, line and column.

**AC-16 · Acceptance Criterion · The error names the file path of the node record.**

**AC-17 · Acceptance Criterion · The error names the 1-based line and 1-based column of the node record.**

###### RQ-59 · While formatting, the reader shall accept a keyed record without a key.

**AC-207 · Acceptance Criterion · A tree whose nodes lack keys is read without error when the formatter reads it.**

**AC-208 · Acceptance Criterion · Every role other than the formatter receives the error of a node without a key.**

**AC-209 · Acceptance Criterion · A node read without a key has no key until the formatter mints one.**

###### RQ-60 · If a node's key is not an RFC 9562 UUID in lowercase hyphenated form, then the reader shall report an error naming the file, line and column.

**AC-210 · Acceptance Criterion · The error names the offending key.**

**AC-211 · Acceptance Criterion · The error names the file, line and column of the `key` field.**

**AC-212 · Acceptance Criterion · A key written in uppercase hexadecimal is an error.**

**AC-213 · Acceptance Criterion · A key without hyphens is an error.**

###### RQ-218 · When formatting a decision, driver, option, consequence or registry entry without a key, the formatter shall mint a version 7 UUID as its key.

**AC-844 · Acceptance Criterion · A decision without `key` has one after formatting.**

**AC-845 · Acceptance Criterion · A driver, an option or a consequence without `key` has one after formatting.**

**AC-846 · Acceptance Criterion · A tag or category entry without `key` has one after formatting.**

**AC-847 · Acceptance Criterion · The keyed records are nodes, decisions, drivers, options, consequences, tag entries and category entries.**

##### F-2 · Labels and aliases

Human-readable labels allocated at publish from per-prefix counters, recorded in the bindings file, and preserved as aliases when a node's label changes.

###### RQ-6 · The writer shall record labels only in the bindings file.

**AC-19 · Acceptance Criterion · No node record under `spec/` carries a label field.**

**AC-20 · Acceptance Criterion · `spec/labels.yaml` maps each labelled key to exactly one label.**

**AC-21 · Acceptance Criterion · A node authored since the last publish or conversion has no entry in `spec/labels.yaml`.**

###### RQ-7 · The publisher shall allocate each label from a per-prefix counter.

**AC-22 · Acceptance Criterion · A label is a kind prefix, a hyphen and a positive integer without leading zeros.**

**AC-23 · Acceptance Criterion · The next value of each prefix counter is stored in `spec/labels.yaml`.**

**AC-24 · Acceptance Criterion · Labels are allocated to unlabelled nodes in document order.**

**AC-243 · Acceptance Criterion · The kind prefixes are `P` for a Product, `FG` for a Feature Group, `F` for a Feature, `RQ` for a Requirement, `AC` for a criterion, `CMP` for a Component, `CON` for a Constraint, `QA` for a Quality Attribute and `ADR` for a decision.**

**AC-262 · Acceptance Criterion · Root Components are visited in the order of their file names when labels are allocated.**

###### RQ-8 · The publisher shall never rewind a label counter.

**AC-25 · Acceptance Criterion · A label counter's next value never decreases between two publishes.**

**AC-26 · Acceptance Criterion · A label of a deleted node is never allocated again.**

###### RQ-9 · When a node's label changes, the publisher shall record the previous label as a permanent alias of the same key.

**AC-27 · Acceptance Criterion · The previous label appears in the key's alias list after the change.**

**AC-28 · Acceptance Criterion · An alias list only grows.**

**AC-29 · Acceptance Criterion · An alias resolves to the key it was assigned to.**

**AC-31 · Acceptance Criterion · A Requirement promoted to a Feature keeps its Requirement label as an alias.**

###### RQ-10 · If an alias equals a live label of another key, then the validator shall report an error.

**AC-32 · Acceptance Criterion · The error names both keys.**

**AC-33 · Acceptance Criterion · The error names the colliding label.**

###### RQ-63 · When a published node's kind changes, the publisher shall allocate it a label of the new kind's prefix.

**AC-223 · Acceptance Criterion · A Requirement promoted to a Feature receives the next `F` label at the next publish.**

**AC-224 · Acceptance Criterion · The node's previous label becomes an alias of its key.**

**AC-225 · Acceptance Criterion · A node whose kind is unchanged keeps its label.**

###### RQ-66 · If an alias is listed under two keys, then the validator shall report an error naming both keys.

**AC-232 · Acceptance Criterion · The error names both keys.**

**AC-233 · Acceptance Criterion · The error names the shared alias.**

###### RQ-70 · If two keys carry the same live label, then the validator shall report an error naming both keys.

**AC-287 · Acceptance Criterion · The error names both keys.**

**AC-288 · Acceptance Criterion · The error names the shared label.**

**AC-289 · Acceptance Criterion · Distinct live labels raise no such error.**

###### RQ-178 · Where a retired or superseded decision names a node the document no longer holds, the writer shall write that reference as the node's former label.

**AC-707 · Acceptance Criterion · A former label is the live label the node carried when it left the document.**

**AC-708 · Acceptance Criterion · A `subject` edge to such a node has the former label as its target.**

**AC-709 · Acceptance Criterion · A driver that names such a node names its former label.**

**AC-710 · Acceptance Criterion · A consequence that names such a node names its former label.**

**AC-809 · Acceptance Criterion · In a file, a former label is written as a mapping whose single field `formerLabel` holds the label.**

##### F-3 · Content view and node hash

What a node's content hash covers and excludes, and how each digest is derived from a domain string and the canonical JSON of a view.

**FT-4 · Facet Tag · portable**

###### RQ-11 · The hasher shall compute a node's content hash from its content view alone.

**AC-35 · Acceptance Criterion · The content view excludes the key.**

**AC-36 · Acceptance Criterion · The content view excludes the label and the aliases.**

**AC-37 · Acceptance Criterion · The content view excludes the node's position among its siblings.**

**AC-38 · Acceptance Criterion · The content view excludes the document key.**

**AC-39 · Acceptance Criterion · The content view excludes every confirmation flag.**

**AC-40 · Acceptance Criterion · Two nodes with equal content views and different keys have the same content hash.**

###### RQ-159 · The hasher shall build a node's content view from the fields the Format lists, in the Format's order.

**AC-596 · Acceptance Criterion · The content view holds, in this order: kind, statement or name, body, the set of tag names sorted by UTF-8 bytes, the typed attributes (quality attribute kind, quality target, constraint binding, conservation kind, verification fate, component boundary), then the ordered list of child content hashes.**

**AC-597 · Acceptance Criterion · Changing a node's body changes its content hash.**

**AC-598 · Acceptance Criterion · A statement enters the content view as its rendered sentence, whether it is stored as `text` or as `form` with slots.**

**AC-599 · Acceptance Criterion · An absent body enters the content view as `null`.**

**AC-600 · Acceptance Criterion · An absent typed attribute enters the content view as `null`.**

**AC-601 · Acceptance Criterion · An empty body and an absent body give different content views.**

**AC-885 · Acceptance Criterion · A Component's boundary enters the content view as its `boundary` field, `null` when the field is absent.**

###### RQ-160 · The hasher shall list a node's child content hashes grouped by kind.

**AC-602 · Acceptance Criterion · Changing a child's content changes its parent's content hash.**

**AC-603 · Acceptance Criterion · Reordering a node's children changes that node's content hash.**

**AC-604 · Acceptance Criterion · Reordering a node's children changes no sibling's content hash.**

**AC-605 · Acceptance Criterion · A node's Sections follow its other children in the list of child content hashes.**

**AC-606 · Acceptance Criterion · A Feature Group's Features precede its Constraints in its list of child content hashes.**

**AC-607 · Acceptance Criterion · A Feature's Requirements precede its Quality Attributes in its list of child content hashes.**

**AC-608 · Acceptance Criterion · A Component's Components precede its Constraints in its list of child content hashes.**

**AC-609 · Acceptance Criterion · A Component's Constraints precede its Quality Attributes in its list of child content hashes.**

###### RQ-12 · The hasher shall derive each digest as SHA-256 over a domain string, one zero byte and the RFC 8785 canonical JSON of the view.

**AC-45 · Acceptance Criterion · The domain strings are `loftspec/node/1`, `loftspec/edges/1`, `loftspec/registry/1`, `loftspec/decision/1`, `loftspec/decisions/1`, `loftspec/content/1`, `loftspec/bindings/1` and `loftspec/state/1`.**

**AC-46 · Acceptance Criterion · The same view hashed under two domain strings yields two different digests.**

**AC-47 · Acceptance Criterion · The canonical bytes of a view are byte-identical to RFC 8785's serialisation of it.**

**AC-48 · Acceptance Criterion · The golden vectors published with the Format are reproduced byte for byte.**

**AC-267 · Acceptance Criterion · Every view whose fields have an order is a JSON array.**

##### F-4 · Roots

The edges root, the content root, the binding root, the registry roots, the decisions root and the state root, and what each of them is sensitive to.

**FT-5 · Facet Tag · portable**

###### RQ-13 · The hasher shall compute the edges root over the sorted multiset of edges, each as its source's hash, its type, its target's hash and its role.

**AC-49 · Acceptance Criterion · An edge enters the multiset by the content hashes of its endpoints.**

**AC-50 · Acceptance Criterion · Two identical edges between duplicate nodes both count.**

**AC-51 · Acceptance Criterion · Adding an edge changes the edges root.**

**AC-52 · Acceptance Criterion · Rekeying a node leaves the edges root unchanged.**

**AC-53 · Acceptance Criterion · Relabelling a node leaves the edges root unchanged.**

**AC-268 · Acceptance Criterion · A node endpoint enters by its content hash.**

**AC-269 · Acceptance Criterion · A decision endpoint enters by its decision view hash.**

**AC-270 · Acceptance Criterion · The edge tuples are sorted by their canonical bytes.**

**AC-643 · Acceptance Criterion · An edge without a role enters the edges root with `null` as its role.**

**AC-711 · Acceptance Criterion · A former-label endpoint enters the edges root as the object `{"formerLabel": <label>}`.**

###### RQ-14 · The hasher shall compute the content root from the Product roots in document order, the Component roots sorted, the edges root, the registry roots and the decisions root.

**AC-54 · Acceptance Criterion · The same document with fresh keys has the same content root.**

**AC-55 · Acceptance Criterion · Relabelling every node leaves the content root unchanged.**

**AC-56 · Acceptance Criterion · Reordering the Products in the manifest's list changes the content root.**

**AC-57 · Acceptance Criterion · Reordering the root Components leaves the content root unchanged.**

**AC-58 · Acceptance Criterion · Changing a registry entry changes the content root.**

**AC-252 · Acceptance Criterion · Adding an edge changes the content root.**

**AC-253 · Acceptance Criterion · Changing a decision changes the content root.**

**AC-254 · Acceptance Criterion · The root Component roots are sorted by their content hash.**

###### RQ-15 · The hasher shall compute the binding root over the sorted bindings of key, label, aliases and content hash.

**AC-59 · Acceptance Criterion · Relabelling a node changes the binding root.**

**AC-240 · Acceptance Criterion · The hash in a binding is computed from the document, never read from `spec/labels.yaml`.**

**AC-271 · Acceptance Criterion · A node's binding carries the node's content hash.**

**AC-272 · Acceptance Criterion · A decision's binding carries the decision view hash.**

**AC-273 · Acceptance Criterion · The bindings are sorted by key in byte order.**

###### RQ-16 · The hasher shall compute the state root over the state view alone.

**AC-62 · Acceptance Criterion · The state view holds the format, the algorithm, the document key, the content root, the binding root and the derived-from record.**

**AC-63 · Acceptance Criterion · Changing any state view field changes the state root.**

**AC-64 · Acceptance Criterion · Two documents with equal state views have the same state root.**

**AC-274 · Acceptance Criterion · The derived-from record holds the source `authoredStateId`, `contentAddress` and `publishSequence`.**

**AC-275 · Acceptance Criterion · A document that is not derived has `null` as its derived-from record.**

**AC-635 · Acceptance Criterion · The derived-from record is read from `spec/loftspec.yaml`.**

###### RQ-61 · The hasher shall compute each registry root over the sorted entries of that registry.

**AC-214 · Acceptance Criterion · The tag registry root is computed over the entries sorted by name in UTF-8 byte order.**

**AC-215 · Acceptance Criterion · The constraint category root is computed over the categories sorted by name in UTF-8 byte order.**

**AC-216 · Acceptance Criterion · Changing an entry's definition changes its registry root.**

**AC-217 · Acceptance Criterion · Reordering the entries in the file leaves the registry root unchanged.**

**AC-278 · Acceptance Criterion · A registry root view begins with the registry's name, `tags` or `categories`.**

**AC-279 · Acceptance Criterion · Two registries with equal entries have different roots.**

###### RQ-62 · The hasher shall compute the decisions root over the sorted decision views.

**AC-221 · Acceptance Criterion · Changing a decision's outcome changes the decisions root.**

**AC-222 · Acceptance Criterion · Reordering the decision files leaves the decisions root unchanged.**

**AC-280 · Acceptance Criterion · Each decision view is digested under `loftspec/decision/1`.**

**AC-281 · Acceptance Criterion · The decisions root is computed over the decision view hashes sorted in byte order.**

###### RQ-161 · The hasher shall build each decision view from the decision's content alone.

**AC-610 · Acceptance Criterion · A decision view holds the title, the context, the state, the drivers, the options, the assessments and the consequences, in that order.**

**AC-611 · Acceptance Criterion · A decision view excludes the keys of the decision and its elements.**

**AC-612 · Acceptance Criterion · The same decision with fresh element keys has the same decision view hash.**

**AC-613 · Acceptance Criterion · A decision view names its selected option by the option's position.**

**AC-614 · Acceptance Criterion · An assessment names its option and its driver by their positions.**

**AC-615 · Acceptance Criterion · The assessments are sorted by option position and then by driver position.**

**AC-616 · Acceptance Criterion · A successor decision is named by its decision view hash.**

**AC-617 · Acceptance Criterion · A driver or consequence that names a node names it by its content hash.**

**AC-712 · Acceptance Criterion · A driver or consequence that names a former label names it as the object `{"formerLabel": <label>}`.**

##### F-5 · Manifest, lock and derived state

The manifest that names the format and the algorithm, the lock that pins the roots at publish, and the record of the source state for a document derived from a Loft v1 export.

###### RQ-17 · The writer shall write a manifest naming the format and the algorithm.

**AC-65 · Acceptance Criterion · `spec/loftspec.yaml` names the format as `loftspec/1`.**

**AC-66 · Acceptance Criterion · `spec/loftspec.yaml` names the algorithm as `sha256-jcs/1`.**

**AC-67 · Acceptance Criterion · `spec/loftspec.yaml` carries the document key.**

**AC-68 · Acceptance Criterion · The document key is unchanged after the first write.**

**AC-241 · Acceptance Criterion · `spec/loftspec.yaml` lists the keys of the Products in document order.**

**AC-276 · Acceptance Criterion · `spec/loftspec.yaml` names the publish branch in `publishBranch` when it is not `main`.**

**AC-634 · Acceptance Criterion · `spec/loftspec.yaml` carries `derivedFrom` for a document derived from a Loft v1 export.**

###### RQ-18 · If the manifest names a format or an algorithm the reader does not implement, then the reader shall refuse the document.

**AC-69 · Acceptance Criterion · The refusal names the unknown value.**

**AC-70 · Acceptance Criterion · No node is read from a refused document.**

###### RQ-19 · The publisher shall write a lock that pins the state root and the content root.

**AC-71 · Acceptance Criterion · `loft.lock.json` carries `format`, `algorithm`, `documentKey`, `stateRoot`, `contentRoot`, `bindingRoot`, `publishSequence` and `publishedAt`.**

**AC-72 · Acceptance Criterion · The content root in the lock equals the hasher's content root of the published `spec/` tree.**

**AC-73 · Acceptance Criterion · The state root in the lock equals the hasher's state root of the published `spec/` tree.**

**AC-666 · Acceptance Criterion · The lock is written to `loft.lock.json` in the directory that holds `spec/`.**

###### RQ-20 · Where a document is derived from a Loft v1 export, the publisher shall record the source state in the lock.

**FT-6 · Facet Tag · conversion**

**AC-74 · Acceptance Criterion · `derivedFrom` carries the source `authoredStateId`.**

**AC-75 · Acceptance Criterion · `derivedFrom` carries the source `contentAddress`.**

**AC-76 · Acceptance Criterion · A document that is not derived has no `derivedFrom` field.**

**AC-77 · Acceptance Criterion · `derivedFrom` is carried unchanged into every later lock of the document.**

**AC-255 · Acceptance Criterion · `derivedFrom` carries the source `publishSequence`.**

###### RQ-67 · If the manifest is absent, then the reader shall report an error naming the expected path.

**AC-234 · Acceptance Criterion · The error names `spec/loftspec.yaml`.**

**AC-235 · Acceptance Criterion · No node is read from a tree without a manifest.**

#### FG-2 · Text form

The `spec/` tree: its directory layout, order and containment, strict reading without derived data, and the YAML subset in canonical layout.

**FT-7 · Facet Tag · portable**

**FT-8 · Facet Tag · git**

##### F-6 · Directory layout

The paths under `spec/` at which Products, Features, Components, decisions, registries, bindings and the manifest are written, and the slug that fixes a file's path when it is created.

###### RQ-21 · The writer shall write each Product to `spec/products/<slug>/product.yaml`.

**AC-78 · Acceptance Criterion · The Product file holds the Product record.**

**AC-79 · Acceptance Criterion · The Product file holds the Product's Feature Group records in order.**

**AC-80 · Acceptance Criterion · Each Feature Group record lists its Features' keys in order.**

**AC-305 · Acceptance Criterion · A Feature Group's Constraints are written in its record in the Product file.**

###### RQ-22 · The writer shall write each Feature to its own file under its Product's directory.

**AC-81 · Acceptance Criterion · A Feature's path is `spec/products/<product-slug>/<group-slug>/<feature-slug>.yaml`.**

**AC-82 · Acceptance Criterion · A Feature file holds exactly one Feature.**

**AC-83 · Acceptance Criterion · A Feature file holds the Feature's Requirements and their criteria.**

**AC-306 · Acceptance Criterion · A Feature's Quality Attributes are written in its Feature file.**

###### RQ-23 · The writer shall write each root Component with its subtree to `spec/components/<slug>.yaml`.

**AC-84 · Acceptance Criterion · A Component file holds one root Component.**

**AC-85 · Acceptance Criterion · A Component file holds every Component, Constraint and Quality Attribute beneath its root.**

**AC-307 · Acceptance Criterion · A Component record lists its Components, its Constraints and its Quality Attributes under separate keys.**

###### RQ-24 · The writer shall write each decision to `spec/decisions/<slug>.yaml`.

**AC-86 · Acceptance Criterion · A decision file holds one decision.**

**AC-87 · Acceptance Criterion · A decision file holds the decision's drivers, options, assessments and consequences.**

###### RQ-25 · The writer shall write the registries, the bindings and the manifest to their fixed paths.

**AC-88 · Acceptance Criterion · The tag registry is `spec/tags.yaml`.**

**AC-90 · Acceptance Criterion · The constraint categories are `spec/categories.yaml`.**

**AC-91 · Acceptance Criterion · The bindings are `spec/labels.yaml`.**

**AC-92 · Acceptance Criterion · The manifest is `spec/loftspec.yaml`.**

**AC-93 · Acceptance Criterion · No file other than those the Format names lives directly under `spec/`.**

###### RQ-26 · The formatter shall never change a file's path.

**AC-94 · Acceptance Criterion · A node whose name changes keeps its file path.**

**AC-95 · Acceptance Criterion · A file's slug is set when the file is created.**

**AC-245 · Acceptance Criterion · A record's slug is stored in its `slug` field and is the only source of its file or directory name.**

**AC-246 · Acceptance Criterion · The converter derives a record's initial slug from its v1 name deterministically.**

###### RQ-168 · The writer shall write each edge in the record of its source element.

**AC-630 · Acceptance Criterion · A `realizes` edge is written in its Feature's record.**

**AC-631 · Acceptance Criterion · A `scoped\_to` edge is written in its root Component's record.**

**AC-632 · Acceptance Criterion · A `subject` edge is written in its decision's file.**

**AC-633 · Acceptance Criterion · A `mentions` edge is written in its source element's record.**

**AC-803 · Acceptance Criterion · A record's `realizes` field holds its `realizes` edges.**

**AC-804 · Acceptance Criterion · A record's `scopedTo` field holds its `scoped\_to` edges.**

**AC-805 · Acceptance Criterion · A record's `mentions` field holds its `mentions` edges.**

**AC-806 · Acceptance Criterion · A decision's `subjects` field holds its `subject` edges.**

**AC-807 · Acceptance Criterion · The targets in a `realizes`, `scopedTo` or `mentions` field are sorted by key.**

**AC-808 · Acceptance Criterion · A decision's subjects are sorted by role, then by target.**

###### RQ-210 · If a slug is not lowercase ASCII letters and digits in runs joined by single hyphens, then the reader shall report an error naming the file, line and column.

**AC-810 · Acceptance Criterion · A slug such as `node-registry-and-link-mapping` is accepted.**

**AC-811 · Acceptance Criterion · A slug with an uppercase letter is an error.**

**AC-812 · Acceptance Criterion · A slug with a leading, trailing or doubled hyphen is an error.**

**AC-813 · Acceptance Criterion · A slug with a character other than a lowercase ASCII letter, a digit or a hyphen is an error.**

###### RQ-211 · If a record's slug differs from the name of the file or directory it fixes, then the reader shall report an error naming the file, line and column.

**AC-814 · Acceptance Criterion · The error names the slug and the file or directory name.**

**AC-815 · Acceptance Criterion · A slug equal to its file or directory name raises no such error.**

###### RQ-212 · If a Feature file is not in its Feature Group's directory, then the reader shall report an error naming the file.

**AC-816 · Acceptance Criterion · The error names the file and the Feature Group's slug.**

**AC-817 · Acceptance Criterion · A Feature file in its Feature Group's directory raises no such error.**

###### RQ-213 · While formatting, the reader shall take a missing slug from the name of the file or directory that holds the record.

**AC-818 · Acceptance Criterion · A Feature without `slug` takes its file name without `.yaml`.**

**AC-819 · Acceptance Criterion · A Feature Group without `slug` takes the name of its directory.**

**AC-820 · Acceptance Criterion · A Feature Group without `slug` and without a directory takes the slug derived from its name.**

###### RQ-214 · The converter and the formatter shall derive a slug from a name by the Format's slug rule.

**AC-821 · Acceptance Criterion · The letters `A` to `Z` become `a` to `z`, and no other character is lowercased.**

**AC-822 · Acceptance Criterion · Each run of characters other than `a` to `z` and `0` to `9` becomes one hyphen.**

**AC-823 · Acceptance Criterion · Leading and trailing hyphens are removed.**

**AC-824 · Acceptance Criterion · A slug longer than 48 characters is cut to its longest prefix of at most 48 characters that a hyphen follows, or to 48 characters when no such prefix exists.**

**AC-825 · Acceptance Criterion · A name that leaves no character takes its kind as the slug, such as `feature-group`.**

**AC-826 · Acceptance Criterion · A slug equal to a reserved device name (`con`, `prn`, `aux`, `nul`, `com1` to `com9`, `lpt1` to `lpt9`) takes the suffix `-1`.**

**AC-827 · Acceptance Criterion · A slug already used in the same directory takes the first unused suffix of `-2`, `-3` and so on, in document order.**

##### F-7 · Order and containment

How sibling order and the containment of Features in their Feature Group are read from the files themselves rather than from a stored ordinal.

###### RQ-27 · The reader shall take sibling order within a file from list order.

**AC-96 · Acceptance Criterion · Siblings are ordered as they appear in the file.**

**AC-97 · Acceptance Criterion · No record carries an ordinal field.**

###### RQ-28 · The reader shall take the order of Features from their Feature Group's key list.

**AC-98 · Acceptance Criterion · Features are ordered as their keys appear in the Feature Group's list.**

**AC-99 · Acceptance Criterion · A key listed by a Feature Group without a Feature file is an error naming the key.**

**AC-100 · Acceptance Criterion · A Feature file whose key no Feature Group lists is an error naming the file.**

**AC-101 · Acceptance Criterion · A Feature key listed by two Feature Groups is an error naming both groups.**

**AC-102 · Acceptance Criterion · A Feature key listed twice by one Feature Group is an error naming the group.**

###### RQ-175 · The reader shall take the order of Products from the manifest's key list.

**AC-699 · Acceptance Criterion · A key the manifest lists without a Product file is an error naming the key.**

**AC-700 · Acceptance Criterion · A Product file whose key the manifest does not list is an error naming the file.**

**AC-701 · Acceptance Criterion · A Product key listed twice is an error naming the key.**

###### RQ-209 · The reader shall order root Components and decisions by file name.

**AC-801 · Acceptance Criterion · Root Components are ordered by the byte order of their file names.**

**AC-802 · Acceptance Criterion · Decisions are ordered by the byte order of their file names.**

##### F-8 · Strict reading and no derived data

What the reader rejects at a named file, line and column, and the derived values that are never written into a node record.

###### RQ-29 · The writer shall write no derived value into a node record.

**AC-103 · Acceptance Criterion · No node record carries a hash.**

**AC-104 · Acceptance Criterion · No node record carries an ordinal.**

**AC-105 · Acceptance Criterion · No node record carries a label.**

**AC-106 · Acceptance Criterion · No file under `spec/` carries a digest.**

###### RQ-30 · If a record contains a field the Format does not define, then the reader shall report an error naming the file, line and column.

**AC-107 · Acceptance Criterion · The error names the unknown field.**

**AC-108 · Acceptance Criterion · The error names the file, line and column of the unknown field.**

###### RQ-31 · If a mapping repeats a field, then the reader shall report an error naming the file, line and column.

**AC-109 · Acceptance Criterion · The error names the repeated field.**

**AC-110 · Acceptance Criterion · The error names the file, line and column of the second occurrence.**

###### RQ-32 · If indentation contains a tab, then the reader shall report an error naming the file, line and column.

**AC-111 · Acceptance Criterion · The error names the file, line and column of the tab.**

###### RQ-65 · If a file is not valid UTF-8, then the reader shall report an error naming the file and the byte offset.

**AC-229 · Acceptance Criterion · The error names the file path.**

**AC-230 · Acceptance Criterion · The error names the byte offset of the first invalid sequence.**

**AC-231 · Acceptance Criterion · A byte order mark is an error naming the file, line and column.**

###### RQ-71 · If a tag name is not a lowercase letter followed by lowercase letters, digits and hyphens, then the reader shall report an error naming the file, line and column.

**AC-290 · Acceptance Criterion · The error names the offending tag name.**

**AC-291 · Acceptance Criterion · A tag name with an uppercase letter is an error.**

**AC-292 · Acceptance Criterion · A tag name containing a dot is an error.**

**AC-293 · Acceptance Criterion · A tag name such as `conversion` is accepted.**

**AC-694 · Acceptance Criterion · A tag name that begins with a digit or a hyphen is an error.**

###### RQ-72 · If a constraint category name is not one or more segments joined by dots, each a lowercase letter followed by lowercase letters, digits and hyphens, then the reader shall report an error naming the file, line and column.

**AC-294 · Acceptance Criterion · The error names the offending category name.**

**AC-295 · Acceptance Criterion · A category name with an empty segment is an error.**

**AC-296 · Acceptance Criterion · A category name with an uppercase letter is an error.**

**AC-297 · Acceptance Criterion · A category name such as `runtime.language` is accepted.**

**AC-695 · Acceptance Criterion · A category name segment that begins with a digit or a hyphen is an error.**

###### RQ-73 · The reader shall accept exactly the edge types `realizes`, `scoped\_to`, `mentions` and `subject`.

**AC-298 · Acceptance Criterion · An edge of type `realizes`, `scoped\_to`, `mentions` or `subject` is accepted.**

**AC-299 · Acceptance Criterion · An edge of any other type is an error naming the file, line and column.**

**AC-300 · Acceptance Criterion · A decision's supersession is recorded in its state, not as an edge.**

##### F-9 · The YAML subset and canonical layout

The YAML constructs the reader accepts and the single layout the formatter writes, so that a formatted tree has one byte sequence.

**FT-9 · Facet Tag · portable**

###### RQ-33 · The reader shall accept only the YAML subset the Format defines.

**AC-112 · Acceptance Criterion · Block mappings and block sequences are accepted.**

**AC-113 · Acceptance Criterion · Plain scalars, double-quoted scalars and literal block scalars are accepted.**

**AC-114 · Acceptance Criterion · A YAML anchor, alias or tag is an error naming the file, line and column.**

**AC-115 · Acceptance Criterion · A flow collection is an error naming the file, line and column.**

**AC-116 · Acceptance Criterion · A folded or single-quoted scalar is an error naming the file, line and column.**

**AC-117 · Acceptance Criterion · A second document in one stream is an error naming the file, line and column.**

**AC-248 · Acceptance Criterion · A comment is an error naming the file, line and column.**

###### RQ-34 · The formatter shall write each record in the Format's key order.

**AC-118 · Acceptance Criterion · Each record kind's keys appear in the order the Format publishes for that kind.**

**AC-119 · Acceptance Criterion · Formatting the formatter's own output changes no byte.**

###### RQ-35 · The formatter shall write every string in the canonical scalar style.

**AC-121 · Acceptance Criterion · A string is written as a literal block scalar only when it reads back byte for byte.**

**AC-122 · Acceptance Criterion · Any other string is written double-quoted.**

**AC-123 · Acceptance Criterion · A string with trailing spaces reads back byte for byte.**

**AC-124 · Acceptance Criterion · A string with a tab reads back byte for byte.**

**AC-125 · Acceptance Criterion · A string containing any Unicode scalar value reads back byte for byte.**

###### RQ-36 · The formatter shall write each registry sorted.

**AC-126 · Acceptance Criterion · Tag registry entries are sorted by name.**

**AC-128 · Acceptance Criterion · Constraint categories are sorted by name.**

**AC-129 · Acceptance Criterion · Bindings are sorted by key.**

###### RQ-69 · The formatter shall write with two-space indentation.

**AC-238 · Acceptance Criterion · Every indentation step is two spaces.**

**AC-239 · Acceptance Criterion · A sequence item is indented two spaces under its parent key.**

###### RQ-215 · Where a value is a key, a label, a slug, an integer, an enumerated value, a tag name, a category name or a format identifier, the formatter shall write it as a plain scalar.

**AC-828 · Acceptance Criterion · A key is written as a plain scalar.**

**AC-829 · Acceptance Criterion · An integer field is written as a plain scalar.**

**AC-830 · Acceptance Criterion · A value that a YAML 1.1 or 1.2 core schema reads as null, a boolean or a number in a field that is not an integer, such as a tag named `yes`, is written double-quoted.**

**AC-831 · Acceptance Criterion · Free text, such as a name, a body or a statement slot, is never written as a plain scalar.**

###### RQ-216 · The formatter shall write a string as a literal block scalar only when it contains a line feed and reads back byte for byte.

**AC-832 · Acceptance Criterion · A string containing a carriage return, U+0085, U+2028, U+2029 or U+FEFF is written double-quoted.**

**AC-833 · Acceptance Criterion · A string containing a character that is not printable is written double-quoted.**

**AC-834 · Acceptance Criterion · A string whose first line that is not empty begins with a space is written double-quoted.**

**AC-835 · Acceptance Criterion · A string made only of line feeds is written double-quoted.**

**AC-836 · Acceptance Criterion · A literal block scalar of a string without a final line feed has the header `\|-`.**

**AC-837 · Acceptance Criterion · A literal block scalar of a string with exactly one final line feed has the header `\|`.**

**AC-838 · Acceptance Criterion · A literal block scalar of a string with more than one final line feed has the header `\|+`.**

**AC-839 · Acceptance Criterion · A tab after a literal line's indentation is read as content.**

###### RQ-217 · The formatter shall write a double-quoted string on one line with the Format's escapes.

**AC-840 · Acceptance Criterion · A backslash is written `\\\\` and a double quote `\\"`.**

**AC-841 · Acceptance Criterion · A line feed is written `\\n`, a tab `\\t`, a carriage return `\\r` and U+0000 `\\0`.**

**AC-842 · Acceptance Criterion · Any other character that is not printable is written as `\\u` and four uppercase hexadecimal digits.**

**AC-843 · Acceptance Criterion · A character is printable when it is in U+0020 to U+007E, U+00A0 to U+D7FF, U+E000 to U+FFFD or U+10000 to U+10FFFF, other than U+2028, U+2029 and U+FEFF.**

##### F-49 · Record fields

The fields each record of the text form holds, their order, and which of them are required.

###### RQ-186 · The writer shall write each Product record with the Product fields in the Format's order.

**AC-743 · Acceptance Criterion · A Product record's fields are, in order, `key`, `slug`, `name`, `body`, `tags`, `mentions`, `featureGroups` and `sections`.**

**AC-744 · Acceptance Criterion · A Product record requires `key`, `slug` and `name`.**

###### RQ-187 · The writer shall write each Feature Group record with the Feature Group fields in the Format's order.

**AC-745 · Acceptance Criterion · A Feature Group record's fields are, in order, `key`, `slug`, `name`, `body`, `tags`, `mentions`, `features`, `constraints` and `sections`.**

**AC-746 · Acceptance Criterion · A Feature Group record requires `key`, `slug` and `name`.**

###### RQ-188 · The writer shall write each Feature record with the Feature fields in the Format's order.

**AC-747 · Acceptance Criterion · A Feature record's fields are, in order, `key`, `slug`, `name`, `body`, `tags`, `realizes`, `mentions`, `requirements`, `qualityAttributes` and `sections`.**

**AC-748 · Acceptance Criterion · A Feature record requires `key`, `slug` and `name`.**

###### RQ-189 · The writer shall write each Requirement record with the Requirement fields in the Format's order.

**AC-749 · Acceptance Criterion · A Requirement record's fields are, in order, `key`, `statement`, `body`, `tags`, `mentions`, `criteria` and `sections`.**

**AC-750 · Acceptance Criterion · A Requirement record requires `key` and `statement`.**

###### RQ-190 · The writer shall write each statement record with the statement fields in the Format's order.

**AC-751 · Acceptance Criterion · A statement stored as text has the single field `text`.**

**AC-752 · Acceptance Criterion · A statement stored as a form has `form`, then the condition slot `when`, `while`, `if` or `where` where the form takes one, then `system` and `shall`.**

###### RQ-191 · The writer shall write each criterion record with the criterion fields in the Format's order.

**AC-753 · Acceptance Criterion · A criterion record's fields are, in order, `key`, `name`, `body`, `tags`, `mentions` and `sections`.**

**AC-754 · Acceptance Criterion · A criterion record requires `key` and `name`.**

###### RQ-192 · The writer shall write each Component record with the Component fields in the Format's order.

**AC-755 · Acceptance Criterion · A Component record's fields are, in order, `key`, `name`, `body`, `tags`, `boundary`, `scopedTo`, `mentions`, `components`, `constraints`, `qualityAttributes` and `sections`.**

**AC-756 · Acceptance Criterion · A Component record requires `key` and `name`.**

**AC-757 · Acceptance Criterion · A root Component record carries `slug` after `key` and requires it.**

**AC-758 · Acceptance Criterion · A Component record that is not a root carries no `slug`.**

###### RQ-193 · The writer shall write each Constraint record with the Constraint fields in the Format's order.

**AC-759 · Acceptance Criterion · A Constraint record's fields are, in order, `key`, `name`, `body`, `tags`, `constraintBinding`, `conservationKind`, `verificationFate`, `mentions` and `sections`.**

**AC-760 · Acceptance Criterion · A Constraint record requires `key` and `name`.**

**AC-761 · Acceptance Criterion · A `constraintBinding` holds `category` and then `requiredValue`, and requires both.**

###### RQ-194 · The writer shall write each Quality Attribute record with the Quality Attribute fields in the Format's order.

**AC-762 · Acceptance Criterion · A Quality Attribute record's fields are, in order, `key`, `name`, `body`, `tags`, `qaKind`, `qualityTarget`, `verificationFate`, `mentions` and `sections`.**

**AC-763 · Acceptance Criterion · A Quality Attribute record requires `key`, `name`, `qaKind` and `qualityTarget`.**

**AC-764 · Acceptance Criterion · A `qualityTarget` holds, in order, `metric`, `band`, `unit`, `aggregation`, `scenario` and `environment`, and requires all of them.**

**AC-765 · Acceptance Criterion · A `band` holds `kind`, then whichever of `lower`, `upper` and `value` its kind takes, in that order.**

###### RQ-195 · The writer shall write each Section record with the Section fields in the Format's order.

**AC-766 · Acceptance Criterion · A Section record's fields are, in order, `key`, `name`, `body`, `mentions` and `sections`.**

**AC-767 · Acceptance Criterion · A Section record requires `key`, `name` and `body`.**

###### RQ-196 · The writer shall write each decision record with the decision fields in the Format's order.

**AC-768 · Acceptance Criterion · A decision record's fields are, in order, `key`, `slug`, `title`, `context`, `state`, `drivers`, `options`, `assessments`, `consequences`, `subjects` and `mentions`.**

**AC-769 · Acceptance Criterion · A decision record requires `key`, `slug`, `title`, `context` and `state`.**

###### RQ-197 · The writer shall write each decision state with the fields its kind takes in the Format's order.

**AC-770 · Acceptance Criterion · A state record begins with `kind`, one of `proposed`, `accepted`, `rejected`, `superseded` and `retired`.**

**AC-771 · Acceptance Criterion · A `proposed` state holds `kind` alone.**

**AC-772 · Acceptance Criterion · An `accepted` state holds `kind`, `selectedOption` and `outcome`.**

**AC-773 · Acceptance Criterion · A `rejected` state holds `kind` and `rationale`.**

**AC-774 · Acceptance Criterion · A `superseded` state holds `kind`, `selectedOption`, `outcome` and `successor`.**

**AC-775 · Acceptance Criterion · A `retired` state holds `kind`, `selectedOption`, `outcome` and `rationale`.**

###### RQ-198 · The writer shall write each driver record with the fields its kind takes in the Format's order.

**AC-776 · Acceptance Criterion · A local driver's fields are, in order, `key`, `kind`, `label` and `statement`, and all are required.**

**AC-777 · Acceptance Criterion · A subject driver's fields are, in order, `key`, `kind` and `subject`, and all are required.**

###### RQ-199 · The writer shall write each option record with the option fields in the Format's order.

**AC-778 · Acceptance Criterion · An option record's fields are, in order, `key`, `title` and `description`, and all are required.**

###### RQ-200 · The writer shall write each assessment record with the assessment fields in the Format's order.

**AC-779 · Acceptance Criterion · An assessment record's fields are, in order, `option`, `driver`, `rating` and `rationale`, and all are required.**

###### RQ-201 · The writer shall write each consequence record with the consequence fields in the Format's order.

**AC-780 · Acceptance Criterion · A consequence record's fields are, in order, `key`, `kind`, `statement` and `subject`.**

**AC-781 · Acceptance Criterion · A consequence record requires `key`, `kind` and `statement`.**

###### RQ-202 · The writer shall write each subject record with the subject fields in the Format's order.

**AC-782 · Acceptance Criterion · A subject record's fields are, in order, `role` and `node`, and both are required.**

###### RQ-203 · The writer shall write the manifest with the manifest fields in the Format's order.

**AC-783 · Acceptance Criterion · The manifest's fields are, in order, `format`, `algorithm`, `documentKey`, `publishBranch`, `products` and `derivedFrom`.**

**AC-784 · Acceptance Criterion · The manifest requires `format`, `algorithm` and `documentKey`.**

**AC-785 · Acceptance Criterion · `derivedFrom` holds, in order, `authoredStateId`, `contentAddress` and `publishSequence`, and requires all three.**

###### RQ-204 · The writer shall write `spec/labels.yaml` as a mapping of `counters` and `bindings`.

**AC-786 · Acceptance Criterion · `counters` is required and maps each of the nine label prefixes to its next value, in the order `P`, `FG`, `F`, `RQ`, `AC`, `CMP`, `CON`, `QA`, `ADR`.**

**AC-787 · Acceptance Criterion · A binding's fields are, in order, `key`, `label` and `aliases`, and `key` and `label` are required.**

**AC-788 · Acceptance Criterion · A binding's aliases are written in the order they were recorded.**

###### RQ-205 · The writer shall write the tag registry and the constraint categories each as a sequence of entries.

**AC-789 · Acceptance Criterion · An entry's fields are, in order, `key`, `name` and `definition`, and all are required.**

**AC-790 · Acceptance Criterion · Entries with the same name are sorted by key.**

**AC-791 · Acceptance Criterion · A registry with no entries is written as an empty file.**

###### RQ-206 · The writer shall name every field in camelCase.

**AC-792 · Acceptance Criterion · A field name of two or more words is written in camelCase, such as `qualityTarget`.**

**AC-793 · Acceptance Criterion · An enumerated value of two or more words is written in snake\_case, such as `event\_driven`.**

###### RQ-207 · The writer shall write a reference to an element as the element's key.

**AC-794 · Acceptance Criterion · A reference to a node is written as the node's key.**

**AC-795 · Acceptance Criterion · A reference to a decision, an option or a driver is written as its key.**

###### RQ-208 · The writer shall omit an absent optional field and an empty list.

**AC-796 · Acceptance Criterion · An absent body is not written.**

**AC-797 · Acceptance Criterion · An empty body is written as `""`.**

**AC-798 · Acceptance Criterion · An empty list is not written.**

**AC-799 · Acceptance Criterion · The reader reads a missing list as empty.**

**AC-800 · Acceptance Criterion · The reader reads a missing optional field as absent.**

###### RQ-220 · The writer shall write each conflict record with the conflict fields in the Format's order.

**AC-857 · Acceptance Criterion · A conflict record's fields are, in order, `field`, `base`, `ours` and `theirs`.**

**AC-858 · Acceptance Criterion · A conflict record requires `field`.**

**AC-859 · Acceptance Criterion · A side on which the field is absent omits its member.**

**AC-860 · Acceptance Criterion · `base`, `ours` and `theirs` each hold a value in the shape the named field takes.**

**AC-861 · Acceptance Criterion · A conflict over a record's place in its list names the field `position`.**

**AC-862 · Acceptance Criterion · A keyed record carries `conflicts` after every other field.**

##### F-50 · Component boundary

Whether a Component lies inside the document or outside Loft's governance, and how a Component's subtree takes it on.

###### RQ-221 · The reader shall read a Component's boundary as `private` or `external`.

**AC-878 · Acceptance Criterion · A Component record's `boundary` field holds `private` or `external`.**

**AC-879 · Acceptance Criterion · Any other value in `boundary` is an error naming the file, line and column.**

###### RQ-222 · loftspec shall give a Component without a `boundary` field the boundary of its parent Component.

**AC-880 · Acceptance Criterion · A nested Component without `boundary` has its parent Component's boundary.**

**AC-881 · Acceptance Criterion · A root Component without `boundary` has the boundary `private`.**

**AC-882 · Acceptance Criterion · A Component with a `boundary` field has the boundary that the field holds.**

###### RQ-223 · If a Component's `boundary` is `private` and a Component above it has the boundary `external`, then the validator shall report an error.

**AC-883 · Acceptance Criterion · The error names the Component and the nearest Component above it whose boundary is `external`.**

**AC-884 · Acceptance Criterion · A Component whose `boundary` is `external` beneath an `external` Component raises no error.**

#### FG-3 · Statements and bodies

The statement record with the five EARS forms and the render rule, the Markdown subset for bodies, and the Section node for prose that needs headings.

**FT-10 · Facet Tag · portable**

##### F-10 · The statement record

How a Requirement's sentence is stored: as plain text, or as one of the five basic EARS forms with its slots filled.

###### RQ-37 · The writer shall write a Requirement's sentence as a statement record.

**AC-130 · Acceptance Criterion · A statement holds `text` alone, or `form` with the slots of that form.**

**AC-131 · Acceptance Criterion · A statement with both `text` and `form` is an error.**

**AC-132 · Acceptance Criterion · A Requirement without a statement is an error.**

###### RQ-38 · The reader shall accept exactly the five basic EARS forms as statement forms.

**AC-133 · Acceptance Criterion · `ubiquitous` takes the slots `system` and `shall`.**

**AC-134 · Acceptance Criterion · `event\_driven` takes `when`, `system` and `shall`.**

**AC-135 · Acceptance Criterion · `state\_driven` takes `while`, `system` and `shall`.**

**AC-136 · Acceptance Criterion · `unwanted` takes `if`, `system` and `shall`.**

**AC-137 · Acceptance Criterion · `optional` takes `where`, `system` and `shall`.**

**AC-138 · Acceptance Criterion · A form outside the five is an error naming the file, line and column.**

**AC-139 · Acceptance Criterion · A missing slot for the form is an error naming the slot.**

**AC-140 · Acceptance Criterion · A slot the form does not take is an error naming the slot.**

**AC-277 · Acceptance Criterion · The `shall` slot holds the text after the modal verb, its closing punctuation included.**

##### F-11 · Rendering statements

The fixed template that turns each statement form back into one sentence.

###### RQ-40 · The renderer shall render each statement form by its fixed template.

**AC-143 · Acceptance Criterion · `ubiquitous` renders as `<system> shall <shall>`.**

**AC-144 · Acceptance Criterion · `event\_driven` renders as `When <when>, <system> shall <shall>`.**

**AC-145 · Acceptance Criterion · `state\_driven` renders as `While <while>, <system> shall <shall>`.**

**AC-146 · Acceptance Criterion · `unwanted` renders as `If <if>, then <system> shall <shall>`.**

**AC-147 · Acceptance Criterion · `optional` renders as `Where <where>, <system> shall <shall>`.**

**AC-148 · Acceptance Criterion · The `system` slot is rendered verbatim.**

**AC-149 · Acceptance Criterion · Rendering the same statement twice yields identical bytes.**

##### F-12 · Bodies and sections

The Markdown subset a body is written in, the Section node that carries headed prose, and the obligation to keep body text exactly as written.

###### RQ-41 · The reader shall accept a body written in the Format's Markdown subset.

**AC-150 · Acceptance Criterion · Paragraphs, bullet lists and numbered lists are accepted.**

**AC-151 · Acceptance Criterion · Code spans and fenced code blocks are accepted.**

**AC-152 · Acceptance Criterion · Emphasis and links are accepted.**

**AC-153 · Acceptance Criterion · A heading in a body is a lint finding, not a read error.**

**AC-154 · Acceptance Criterion · Raw HTML in a body is a lint finding, not a read error.**

**AC-261 · Acceptance Criterion · Any other Markdown construct is a lint finding, not a read error.**

###### RQ-42 · The writer shall write prose that needs headings as Section nodes.

**AC-155 · Acceptance Criterion · A Section has a name and a body.**

**AC-156 · Acceptance Criterion · A Section has ordered child Sections.**

**AC-157 · Acceptance Criterion · A Section is accepted under any node that has a body.**

**AC-158 · Acceptance Criterion · A Section's content hash is computed by the node rule.**

**AC-260 · Acceptance Criterion · A Section is written inside its owning node's record under `sections`.**

###### RQ-43 · The reader shall preserve body text byte for byte.

**AC-159 · Acceptance Criterion · Trailing whitespace in a body survives a write and a read.**

**AC-160 · Acceptance Criterion · A tab in a body survives a write and a read.**

**AC-161 · Acceptance Criterion · Every Unicode scalar value in a body survives a write and a read.**

#### FG-4 · Conversion from a v1 export

How a Loft v1 export becomes a v2 document: the node, registry and link mapping, statements and references recovered from prose, the sidecar for what the content view excludes, and the round-trip obligation.

**FT-11 · Facet Tag · conversion**

##### F-13 · Node, registry and link mapping

What each v1 node, Vocabulary Entry, Realization Link, Component scope and Architecture Decision becomes in the v2 document.

###### RQ-44 · The converter shall map each v1 node to exactly one v2 node of the corresponding kind.

**AC-162 · Acceptance Criterion · Product, Feature Group, Feature and Requirement tier sections map to `product`, `feature\_group`, `feature` and `requirement`.**

**AC-163 · Acceptance Criterion · An Acceptance Criterion maps to a `criterion`.**

**AC-164 · Acceptance Criterion · A Component maps to a `component`.**

**AC-165 · Acceptance Criterion · A Constraint maps to a `constraint` with its technical binding, conservation kind and verification fate kept field for field.**

**AC-166 · Acceptance Criterion · A Quality Attribute maps to a `quality\_attribute` with its kind and its quality target kept field for field.**

**AC-167 · Acceptance Criterion · The v2 node count equals the v1 node count less the Facet Tag nodes.**

###### RQ-45 · The converter shall keep each v1 node's UUID as its key and its external ID as its label.

**AC-168 · Acceptance Criterion · Every binding's key equals the v1 `nodeId`.**

**AC-169 · Acceptance Criterion · Every binding's label equals the v1 `externalId`.**

**AC-170 · Acceptance Criterion · No alias is created by conversion.**

**AC-244 · Acceptance Criterion · Every converted decision has a binding whose label is its v1 external ID.**

###### RQ-46 · The converter shall take parentage from the v1 children hashes.

**AC-171 · Acceptance Criterion · Each node's parent is the node whose `childrenHashes` lists it.**

###### RQ-47 · The converter shall turn each Facet Tag node into a tag on its parent.

**AC-173 · Acceptance Criterion · The parent lists the tag name.**

**AC-174 · Acceptance Criterion · The Facet Tag node does not exist in the v2 document.**

###### RQ-48 · The converter shall turn each Vocabulary Entry into a tag registry entry.

**AC-175 · Acceptance Criterion · A registry entry keeps the entry's UUID as key.**

**AC-176 · Acceptance Criterion · A registry entry keeps the entry's name and definition.**

###### RQ-49 · The converter shall turn each Realization Link into a `realizes` edge.

**AC-177 · Acceptance Criterion · A `realizes` edge runs from the Feature to the Component.**

**AC-178 · Acceptance Criterion · The edge count equals the v1 Realization Link count.**

###### RQ-50 · The converter shall turn each authored Component scope into a `scoped\_to` edge.

**AC-179 · Acceptance Criterion · A Component scoped to a Product has a `scoped\_to` edge to that Product.**

**AC-180 · Acceptance Criterion · A Component without an authored scope has no `scoped\_to` edge.**

###### RQ-51 · The converter shall turn each Architecture Decision into a decision with driver, option and consequence children, typed assessments and subject edges.

**AC-181 · Acceptance Criterion · Each decision element keeps its v1 UUID as key.**

**AC-182 · Acceptance Criterion · The decision's state is kept.**

**AC-183 · Acceptance Criterion · The decision's selected option is kept.**

**AC-184 · Acceptance Criterion · Each subject link becomes a `subject` edge carrying its role.**

**AC-256 · Acceptance Criterion · The decision's outcome is kept.**

**AC-257 · Acceptance Criterion · Each driver, option, consequence and assessment keeps its fields field for field.**

**AC-723 · Acceptance Criterion · A reference to a node the export does not hold becomes that node's v1 external ID as a former label.**

###### RQ-64 · The converter shall turn each technical constraint category into a constraint category entry.

**AC-226 · Acceptance Criterion · A category entry keeps the category's UUID as key.**

**AC-227 · Acceptance Criterion · A category entry keeps the category's fields field for field.**

**AC-228 · Acceptance Criterion · The category entry count equals the v1 category count.**

###### RQ-68 · The converter shall take sibling order from the v1 ordinals.

**AC-236 · Acceptance Criterion · Siblings are ordered by ascending ordinal.**

**AC-237 · Acceptance Criterion · No ordinal is written into a v2 record.**

###### RQ-176 · The converter shall set each label counter to one more than the highest label value seen for its prefix.

**AC-702 · Acceptance Criterion · A prefix with no label in the export has its counter at 1.**

**AC-721 · Acceptance Criterion · A former label in a decision counts as a label seen for its prefix.**

###### RQ-177 · The converter shall key the document by the v1 document UUID that every element of the export carries.

**AC-704 · Acceptance Criterion · The document key equals the `documentId` of every node, registry entry and decision of the export.**

**AC-705 · Acceptance Criterion · An export whose elements carry two document UUIDs is refused with an error naming both.**

**AC-706 · Acceptance Criterion · An export with no node, registry entry or decision is refused.**

###### RQ-224 · The converter shall give the boundary `external` to each Component that the export scopes as external.

**AC-892 · Acceptance Criterion · A Component that the export scopes as external has the `boundary` field `external`.**

**AC-893 · Acceptance Criterion · A Component that the export scopes to a Product or to the document has no `boundary` field.**

**AC-894 · Acceptance Criterion · A Component scoped to a Product or to the document beneath a Component scoped as external is reported as a finding naming both.**

##### F-14 · Statements and references from prose

What the converter recovers from v1 names and bodies: statement slots where they render back exactly, `mentions` edges for label references, and bodies kept as written.

###### RQ-52 · The converter shall store statement slots only when their rendering reproduces the v1 name byte for byte.

**AC-185 · Acceptance Criterion · A name whose parsed slots render back to the name is stored as slots.**

**AC-186 · Acceptance Criterion · Any other name is stored as `text`.**

**AC-187 · Acceptance Criterion · A name stored as `text` is reported as a finding.**

###### RQ-53 · The converter shall turn each resolvable label reference in prose into a `mentions` edge.

**AC-188 · Acceptance Criterion · References of the forms `P-n`, `FG-n`, `F-n`, `RQ-n`, `AC-n`, `CMP-n`, `CON-n`, `QA-n` and `ADR-n` are recognised.**

**AC-189 · Acceptance Criterion · A reference with a `D-n/` prefix is recognised.**

**AC-190 · Acceptance Criterion · A resolvable reference becomes a `mentions` edge from the node to the referenced node.**

**AC-191 · Acceptance Criterion · A reference whose label does not resolve is reported as a finding naming the node and the reference.**

**AC-891 · Acceptance Criterion · A label-shaped text whose number is above every label of its prefix in the export is not a reference.**

###### RQ-54 · The converter shall store bodies verbatim.

**AC-192 · Acceptance Criterion · A body is stored byte for byte.**

**AC-193 · Acceptance Criterion · A body with a heading is reported as a finding.**

##### F-15 · The sidecar

The file beside `spec/` that keeps every v1 fact the content view excludes, so that the source state stays reproducible.

###### RQ-55 · The converter shall record in a sidecar every v1 fact the content view excludes.

**AC-194 · Acceptance Criterion · The sidecar records each node's v1 ordinal.**

**AC-195 · Acceptance Criterion · The sidecar records each node's `fateConfirmed`.**

**AC-196 · Acceptance Criterion · The sidecar records each Facet Tag node's UUID and ordinal against its tag application.**

**AC-197 · Acceptance Criterion · The sidecar records the v1 document UUID.**

**AC-198 · Acceptance Criterion · The sidecar records, per label prefix, the highest allocated value.**

**AC-258 · Acceptance Criterion · The sidecar records each node's `proposedFate`.**

**AC-259 · Acceptance Criterion · The sidecar records each Component's v1 scope kind.**

**AC-724 · Acceptance Criterion · The sidecar records the v1 node UUID behind each former label.**

###### RQ-56 · The converter shall write the sidecar outside the `spec/` tree.

**AC-200 · Acceptance Criterion · No file under `spec/` carries a sidecar fact.**

**AC-703 · Acceptance Criterion · The sidecar is the file `v1-sidecar.json` beside `spec/`.**

##### F-16 · The round-trip obligation

What the converter preserves so that the source Authored State's content, graph, vocabulary and decision addresses can be recomputed from the v2 document and the sidecar.

**FT-12 · Facet Tag · portable**

###### RQ-57 · The converter shall preserve what is needed to reproduce the source state's content, graph, vocabulary and decision addresses from the v2 document and the sidecar.

**AC-201 · Acceptance Criterion · The source state's content address is reproduced for each fixture export.**

**AC-202 · Acceptance Criterion · The source state's graph address is reproduced for each fixture export.**

**AC-203 · Acceptance Criterion · The source state's vocabulary address is reproduced for each fixture export.**

**AC-204 · Acceptance Criterion · The source state's decision address is reproduced for each fixture export.**

**AC-247 · Acceptance Criterion · The source state's technical constraint category address is reproduced for each fixture export.**

###### RQ-58 · The converter shall produce identical output for identical input.

**AC-205 · Acceptance Criterion · Two conversions of one export write byte-identical `spec/` trees.**

**AC-206 · Acceptance Criterion · Two conversions of one export write byte-identical sidecars.**

#### FG-5 · Validation and lint

The containment and reference checks that decide whether a document is valid, and the structural, language and v2 lint rules with their findings, reports and repository configuration.

**FT-15 · Facet Tag · portable**

##### F-17 · Containment profile

Which kind of node may stand under which parent kind, and the single Product record that each Product file holds.

###### RQ-74 · The validator shall accept a child only where the containment profile allows its kind under its parent's kind.

**AC-308 · Acceptance Criterion · A `product` is accepted only at the root.**

**AC-309 · Acceptance Criterion · A `feature\_group` is accepted only under a `product`.**

**AC-310 · Acceptance Criterion · A `feature` is accepted only under a `feature\_group`.**

**AC-311 · Acceptance Criterion · A `requirement` is accepted only under a `feature`.**

**AC-312 · Acceptance Criterion · A `criterion` is accepted only under a `requirement`.**

**AC-313 · Acceptance Criterion · A `component` is accepted at the root or under a `component`.**

**AC-314 · Acceptance Criterion · A `constraint` is accepted only under a `component` or a `feature\_group`.**

**AC-315 · Acceptance Criterion · A `quality\_attribute` is accepted only under a `component` or a `feature`.**

**AC-316 · Acceptance Criterion · A `section` is accepted wherever a body is accepted.**

**AC-317 · Acceptance Criterion · A child outside the profile is an error naming the child's key and both kinds.**

**AC-863 · Acceptance Criterion · Where the text form cannot hold a child outside the profile, the reader's error at the child's file, line and column is the error.**

###### RQ-75 · The validator shall accept exactly one Product record per Product file.

**AC-318 · Acceptance Criterion · A Product file with two Product records is an error naming the file.**

**AC-319 · Acceptance Criterion · A Product file without a Product record is an error naming the file.**

**AC-692 · Acceptance Criterion · A Product file with one Product record raises no such error.**

##### F-18 · Reference resolution

How edge endpoints and bindings resolve to elements of the document, which kinds each of the four edge types joins, and the refusal of a tag listed twice on one node.

###### RQ-76 · The validator shall require every edge endpoint that is not a former label to resolve to an element of the document.

**AC-320 · Acceptance Criterion · An edge whose source key no node or decision carries is an error naming the edge and the key.**

**AC-321 · Acceptance Criterion · An edge whose target key no node or decision carries is an error naming the edge and the key.**

**AC-693 · Acceptance Criterion · An edge whose endpoints both resolve raises no such error.**

**AC-713 · Acceptance Criterion · A `subject` edge whose target is a former label raises no unresolved-endpoint error.**

###### RQ-77 · The validator shall require each edge type's endpoints to be of the kinds that type joins.

**AC-322 · Acceptance Criterion · A `realizes` edge whose source is not a `feature` is an error naming the edge.**

**AC-323 · Acceptance Criterion · A `realizes` edge whose target is not a `component` is an error naming the edge.**

**AC-324 · Acceptance Criterion · A `scoped\_to` edge whose source is not a `component` is an error naming the edge.**

**AC-325 · Acceptance Criterion · A `scoped\_to` edge whose target is not a `product` is an error naming the edge.**

**AC-326 · Acceptance Criterion · A `subject` edge whose source is not a decision is an error naming the edge.**

**AC-327 · Acceptance Criterion · A `subject` edge whose target is a decision is an error naming the edge.**

**AC-328 · Acceptance Criterion · A `subject` edge without the role `outcome` or `affected` is an error naming the edge.**

**AC-329 · Acceptance Criterion · A `mentions` edge between any two elements is accepted.**

**AC-642 · Acceptance Criterion · A `realizes`, `scoped\_to` or `mentions` edge that carries a role is an error naming the edge.**

**AC-864 · Acceptance Criterion · Where the text form cannot hold an edge between the wrong kinds, the reader's error at the edge's file, line and column is the error.**

###### RQ-78 · The validator shall require every binding to name an element of the document.

**AC-330 · Acceptance Criterion · A binding whose key no node or decision carries is an error naming the key.**

**AC-331 · Acceptance Criterion · A binding whose key a node or decision carries raises no such error.**

###### RQ-79 · If a node lists one tag twice, then the validator shall report an error naming the node's key and the tag.

**AC-332 · Acceptance Criterion · The error names the node's key.**

**AC-333 · Acceptance Criterion · The error names the repeated tag.**

**AC-334 · Acceptance Criterion · A node that lists each tag once raises no such error.**

###### RQ-179 · If a decision that is neither retired nor superseded names a former label, then the validator shall report an error naming the decision and the label.

**AC-714 · Acceptance Criterion · A former label in a proposed decision is an error.**

**AC-715 · Acceptance Criterion · A former label in an accepted decision is an error.**

**AC-716 · Acceptance Criterion · A former label in a rejected decision is an error.**

**AC-717 · Acceptance Criterion · A former label in a retired or superseded decision raises no such error.**

###### RQ-180 · If a former label is the live label or an alias of an element, then the validator shall report an error naming the label and the element.

**AC-718 · Acceptance Criterion · A former label equal to an element's live label is an error.**

**AC-719 · Acceptance Criterion · A former label equal to an element's alias is an error.**

**AC-720 · Acceptance Criterion · A former label that no element carries raises no such error.**

##### F-19 · Structural lint rules

The ten structural rules, SH-001 to SH-010: the shape each one reports and its default severity.

###### RQ-80 · The linter shall report a Feature with no `realizes` edge as `SH-001`.

**AC-335 · Acceptance Criterion · A Feature with no `realizes` edge yields one `SH-001` finding naming the Feature's key.**

**AC-336 · Acceptance Criterion · A Feature with at least one `realizes` edge yields no `SH-001` finding.**

**AC-337 · Acceptance Criterion · The default severity of `SH-001` is warning.**

###### RQ-81 · The linter shall report a leaf Component that no Feature realizes as `SH-002`.

**AC-338 · Acceptance Criterion · A Component with no child Component and no incoming `realizes` edge yields one `SH-002` finding naming its key.**

**AC-339 · Acceptance Criterion · A Component with a child Component yields no `SH-002` finding.**

**AC-340 · Acceptance Criterion · A Component that at least one Feature realizes yields no `SH-002` finding.**

**AC-341 · Acceptance Criterion · The default severity of `SH-002` is info.**

**AC-886 · Acceptance Criterion · A Component whose boundary is `external` yields no `SH-002` finding.**

###### RQ-82 · The linter shall report a Requirement without a criterion as `SH-003`.

**AC-342 · Acceptance Criterion · A Requirement with no criterion yields one `SH-003` finding naming its key.**

**AC-343 · Acceptance Criterion · A Requirement with at least one criterion yields no `SH-003` finding.**

**AC-344 · Acceptance Criterion · The default severity of `SH-003` is warning.**

###### RQ-83 · The linter shall report a Feature without a Requirement as `SH-004`.

**AC-345 · Acceptance Criterion · A Feature with no Requirement yields one `SH-004` finding naming its key.**

**AC-346 · Acceptance Criterion · A Feature with at least one Requirement yields no `SH-004` finding.**

**AC-347 · Acceptance Criterion · The default severity of `SH-004` is warning.**

###### RQ-84 · The linter shall report a Product or Feature Group without children as `SH-005`.

**AC-348 · Acceptance Criterion · A Product with no Feature Group yields one `SH-005` finding naming its key.**

**AC-349 · Acceptance Criterion · A Feature Group with no Feature yields one `SH-005` finding naming its key.**

**AC-350 · Acceptance Criterion · A Product with at least one Feature Group yields no `SH-005` finding.**

**AC-351 · Acceptance Criterion · A Feature Group with at least one Feature yields no `SH-005` finding.**

**AC-352 · Acceptance Criterion · The default severity of `SH-005` is info.**

###### RQ-85 · The linter shall report a tag registry entry that no node applies as `SH-006`.

**AC-353 · Acceptance Criterion · A registry entry that no node lists yields one `SH-006` finding naming the entry.**

**AC-354 · Acceptance Criterion · A registry entry that at least one node lists yields no `SH-006` finding.**

**AC-355 · Acceptance Criterion · The default severity of `SH-006` is info.**

###### RQ-86 · The linter shall report two siblings with one name as `SH-007`.

**AC-356 · Acceptance Criterion · Two siblings whose names are equal after trimming and lowercasing yield one `SH-007` finding naming both keys.**

**AC-357 · Acceptance Criterion · Two nodes with equal names under different parents yield no `SH-007` finding.**

**AC-358 · Acceptance Criterion · The default severity of `SH-007` is warning.**

**AC-871 · Acceptance Criterion · Siblings are compared within one group of kinds: Products, Feature Groups, Features and Requirements together, then Components, Constraints, Quality Attributes, criteria and Sections each alone.**

###### RQ-87 · The linter shall report a Component whose subtree holds no Constraint and no Quality Attribute as `SH-008`.

**AC-359 · Acceptance Criterion · The topmost Component of a subtree that holds no Constraint and no Quality Attribute yields one `SH-008` finding naming its key.**

**AC-870 · Acceptance Criterion · A Component inside a subtree that already yields an `SH-008` finding yields none of its own.**

**AC-360 · Acceptance Criterion · A Component with a Constraint in its subtree yields no `SH-008` finding.**

**AC-361 · Acceptance Criterion · A Component with a Quality Attribute in its subtree yields no `SH-008` finding.**

**AC-362 · Acceptance Criterion · The default severity of `SH-008` is info.**

**AC-887 · Acceptance Criterion · A Component whose boundary is `external` yields no `SH-008` finding.**

**AC-888 · Acceptance Criterion · A Constraint or Quality Attribute under an `external` Component does not clear `SH-008` for a Component above it.**

###### RQ-88 · The linter shall report a tag that names no registry entry as `SH-009`.

**AC-363 · Acceptance Criterion · A tag on a node that matches no tag registry entry yields one `SH-009` finding naming the node's key and the tag.**

**AC-364 · Acceptance Criterion · A tag that matches a registry entry yields no `SH-009` finding.**

**AC-365 · Acceptance Criterion · The default severity of `SH-009` is warning.**

###### RQ-89 · The linter shall report a Component whose name is shaped like a file path as `SH-010`.

**AC-366 · Acceptance Criterion · A Component whose name contains a path separator yields one `SH-010` finding naming its key.**

**AC-367 · Acceptance Criterion · A Component whose name ends in a full stop and one to four ASCII letters or digits yields one `SH-010` finding naming its key.**

**AC-368 · Acceptance Criterion · A Component name with neither yields no `SH-010` finding.**

**AC-369 · Acceptance Criterion · The default severity of `SH-010` is hint.**

##### F-20 · Language lint rules

The nine language rules, LQ-001 to LQ-009, and the texts they read: what each one reports and its default severity.

###### RQ-90 · The linter shall apply the language rules to assertion texts and to bodies.

**AC-370 · Acceptance Criterion · A finding on a statement names the Requirement's key.**

**AC-371 · Acceptance Criterion · A finding in a body names the node's key and the body line.**

**AC-372 · Acceptance Criterion · A finding on a criterion names the criterion's key.**

**AC-646 · Acceptance Criterion · Texts are split into tokens by the tokenizer the Format publishes.**

**AC-651 · Acceptance Criterion · Every language rule except `LQ-007` reads a criterion's text, a Constraint's name and a Quality Attribute's name as it reads a statement.**

**AC-872 · Acceptance Criterion · A language rule reads the body of the node whose name or statement it reads.**

###### RQ-91 · The linter shall report a weak term as `LQ-001`.

**AC-373 · Acceptance Criterion · A statement or body containing a term of the weak-term lexicon yields one `LQ-001` finding per occurrence, naming the node and the term.**

**AC-374 · Acceptance Criterion · The default weak-term lexicon is published with the Format.**

**AC-375 · Acceptance Criterion · A term removed from the lexicon by configuration yields no `LQ-001` finding.**

**AC-376 · Acceptance Criterion · The default severity of `LQ-001` is warning.**

###### RQ-92 · The linter shall report a vague quantifier as `LQ-002`.

**AC-377 · Acceptance Criterion · A statement or body containing a term of the vague-quantifier lexicon yields one `LQ-002` finding per occurrence, naming the node and the term.**

**AC-378 · Acceptance Criterion · `at most 8`, and the same two words before any other number, yield no `LQ-002` finding.**

**AC-379 · Acceptance Criterion · A number written with digit grouping, such as `65,536`, yields no `LQ-002` finding.**

**AC-380 · Acceptance Criterion · The default severity of `LQ-002` is warning.**

**AC-647 · Acceptance Criterion · The default vague-quantifier lexicon is published with the Format.**

###### RQ-93 · The linter shall report an escape clause as `LQ-003`.

**AC-381 · Acceptance Criterion · A statement containing a clause of the escape-clause lexicon yields one `LQ-003` finding per occurrence, naming the node and the clause.**

**AC-382 · Acceptance Criterion · A statement without such a clause yields no `LQ-003` finding.**

**AC-383 · Acceptance Criterion · The default severity of `LQ-003` is warning.**

**AC-648 · Acceptance Criterion · The default escape-clause lexicon is published with the Format.**

###### RQ-94 · The linter shall report absolute language as `LQ-004`.

**AC-384 · Acceptance Criterion · A statement containing a term of the absolute-language lexicon yields one `LQ-004` finding per occurrence, naming the node and the term, when the rule is enabled.**

**AC-385 · Acceptance Criterion · `LQ-004` is disabled by default.**

**AC-386 · Acceptance Criterion · The default severity of `LQ-004` is info.**

**AC-649 · Acceptance Criterion · The default absolute-language lexicon is published with the Format.**

###### RQ-95 · The linter shall report a compound statement as `LQ-005`.

**AC-387 · Acceptance Criterion · A statement with more than one modal verb yields one `LQ-005` finding naming the node.**

**AC-388 · Acceptance Criterion · A statement whose coordinating conjunctions reach the configured threshold yields one `LQ-005` finding naming the node.**

**AC-389 · Acceptance Criterion · The default threshold is published with the Format.**

**AC-390 · Acceptance Criterion · The default severity of `LQ-005` is info.**

**AC-650 · Acceptance Criterion · The default modal and conjunction lexicons are published with the Format.**

###### RQ-96 · The linter shall report a Quality Attribute without a quantified target as `LQ-006`.

**AC-391 · Acceptance Criterion · A Quality Attribute without a quality target yields one `LQ-006` finding naming its key.**

**AC-392 · Acceptance Criterion · A Quality Attribute with a metric, a band and a unit yields no `LQ-006` finding.**

**AC-393 · Acceptance Criterion · The default severity of `LQ-006` is warning.**

###### RQ-97 · The linter shall report a statement outside the five EARS forms as `LQ-007`.

**AC-394 · Acceptance Criterion · A statement stored as text that parses into none of the five forms yields one `LQ-007` finding naming the node when the rule is enabled.**

**AC-395 · Acceptance Criterion · A statement stored with slots yields no `LQ-007` finding.**

**AC-396 · Acceptance Criterion · A text statement whose subject is a bare proper name parses as a form.**

**AC-397 · Acceptance Criterion · `LQ-007` is disabled by default.**

**AC-398 · Acceptance Criterion · The default severity of `LQ-007` is info.**

###### RQ-98 · The linter shall report two near-duplicate names as `LQ-008`.

**AC-399 · Acceptance Criterion · Two names whose similarity reaches the configured threshold yield one `LQ-008` finding naming both keys.**

**AC-400 · Acceptance Criterion · A name shorter than the configured minimum token count takes part in no `LQ-008` finding.**

**AC-401 · Acceptance Criterion · The default threshold and minimum are published with the Format.**

**AC-402 · Acceptance Criterion · The default severity of `LQ-008` is info.**

**AC-644 · Acceptance Criterion · The similarity of two names is the size of the multiset intersection of their tokens divided by the size of their multiset union.**

**AC-645 · Acceptance Criterion · Names are compared only between two nodes of one kind.**

###### RQ-99 · The linter shall report a tag registry term used in prose by a node that does not list the tag as `LQ-009`.

**AC-403 · Acceptance Criterion · A node whose name or body contains a registry term and whose tags omit it yields one `LQ-009` finding per term, naming the node and the term.**

**AC-404 · Acceptance Criterion · A node that lists the tag, or that sits under a node listing it, yields no `LQ-009` finding.**

**AC-405 · Acceptance Criterion · The default severity of `LQ-009` is hint.**

**AC-873 · Acceptance Criterion · `LQ-009` reads the names and bodies of Products, Feature Groups, Features, Requirements and Components.**

##### F-21 · Lint rules of v2

The three rules the v2 text form adds, V2-001 to V2-003: a heading inside a body, a statement stored as text without slots, and a label reference without a `mentions` edge.

###### RQ-100 · The linter shall report a heading inside a body as `V2-001`.

**AC-406 · Acceptance Criterion · Each Markdown heading line in a body yields one `V2-001` finding naming the node and the line.**

**AC-407 · Acceptance Criterion · A body without a heading yields no `V2-001` finding.**

**AC-408 · Acceptance Criterion · The default severity of `V2-001` is info.**

###### RQ-101 · The linter shall report a statement stored as text without slots as `V2-002`.

**AC-409 · Acceptance Criterion · A statement with `text` and no `form` yields one `V2-002` finding naming the Requirement.**

**AC-410 · Acceptance Criterion · A statement with `form` yields no `V2-002` finding.**

**AC-411 · Acceptance Criterion · The default severity of `V2-002` is info.**

###### RQ-102 · The linter shall report a label reference in prose that has no matching `mentions` edge as `V2-003`.

**AC-412 · Acceptance Criterion · A label-shaped reference in a body or statement without a `mentions` edge from the node to the referenced key yields one `V2-003` finding naming the node and the reference.**

**AC-413 · Acceptance Criterion · A reference with a matching `mentions` edge yields no `V2-003` finding.**

**AC-414 · Acceptance Criterion · The default severity of `V2-003` is warning.**

**AC-874 · Acceptance Criterion · A label-shaped reference whose number is below its prefix's counter and that names no element yields one `V2-003` finding naming the node and the reference.**

**AC-890 · Acceptance Criterion · A label-shaped text whose number is not below its prefix's counter in `spec/labels.yaml` is not a reference.**

##### F-22 · Findings and reports

The typed record that carries each finding and the stable order in which a report lists findings.

###### RQ-103 · The linter shall emit each finding as a typed record.

**AC-415 · Acceptance Criterion · A finding carries the rule id.**

**AC-416 · Acceptance Criterion · A finding carries the severity.**

**AC-417 · Acceptance Criterion · A finding carries the key of each element it names.**

**AC-418 · Acceptance Criterion · A finding carries the file path of each element it names.**

**AC-419 · Acceptance Criterion · A finding carries the line and column where one applies.**

###### RQ-104 · The linter shall emit findings in a stable order.

**AC-420 · Acceptance Criterion · Findings are ordered by file path, then line, then column, then rule id, then the keys they name.**

**AC-421 · Acceptance Criterion · Two runs over one tree emit byte-identical reports.**

##### F-23 · Lint configuration in the repository

The `spec/lint.yaml` file, which disables rules, sets severities, adjusts lexicons and records suppressions, and the guarantee that findings depend only on the tree and that file.

###### RQ-105 · The linter shall read its configuration from `spec/lint.yaml`.

**AC-422 · Acceptance Criterion · A rule can be disabled.**

**AC-423 · Acceptance Criterion · A rule's severity can be set to warning, info or hint.**

**AC-424 · Acceptance Criterion · A rule's lexicon can be extended or reduced where the rule has one.**

**AC-425 · Acceptance Criterion · A suppression names a rule, a key and a reason.**

**AC-426 · Acceptance Criterion · Without `spec/lint.yaml` the default configuration applies.**

**AC-675 · Acceptance Criterion · A finding whose rule and key match a suppression is not reported.**

**AC-698 · Acceptance Criterion · A rule's threshold can be set where the rule has one.**

**AC-866 · Acceptance Criterion · `spec/lint.yaml` lists rules under `rules`, each entry naming its rule in `rule`.**

**AC-867 · Acceptance Criterion · `spec/lint.yaml` lists suppressions under `suppressions`, each entry holding `rule`, `key` and `reason`.**

**AC-868 · Acceptance Criterion · A lexicon is edited either by `replace` or by `extend` and `remove`.**

###### RQ-106 · The linter shall produce the same findings for the same tree and configuration on any machine.

**AC-427 · Acceptance Criterion · The report does not depend on the machine's locale.**

**AC-428 · Acceptance Criterion · The report does not depend on environment variables.**

**AC-676 · Acceptance Criterion · No command-line option changes the findings.**

**AC-677 · Acceptance Criterion · No file outside the tree changes the findings.**

###### RQ-174 · If `spec/lint.yaml` names a rule or a severity the Format does not define, then the linter shall report an error naming the file, line and column.

**AC-672 · Acceptance Criterion · An unknown rule id is reported by name.**

**AC-673 · Acceptance Criterion · A severity outside warning, info and hint is reported by value.**

**AC-674 · Acceptance Criterion · No finding is reported from a tree whose lint configuration is in error.**

**AC-869 · Acceptance Criterion · An option the named rule does not take is an error naming the file, line and column.**

##### F-48 · Decision integrity

The rules a decision's own references, its successor and its completeness for its state must meet.

###### RQ-181 · The validator shall require every option and driver a decision names to be one of that decision's own.

**AC-725 · Acceptance Criterion · A selected option that is not one of the decision's options is an error naming the decision.**

**AC-726 · Acceptance Criterion · An assessment whose option is not one of the decision's options is an error naming the decision and the option.**

**AC-727 · Acceptance Criterion · An assessment whose driver is not one of the decision's drivers is an error naming the decision and the driver.**

**AC-728 · Acceptance Criterion · A decision whose named options and drivers are all its own raises no such error.**

###### RQ-182 · The validator shall require a superseded decision's successor to be another decision of the document.

**AC-729 · Acceptance Criterion · A successor that no decision carries is an error naming the decision and the successor's key.**

**AC-730 · Acceptance Criterion · A decision that names itself as its successor is an error naming the decision.**

**AC-731 · Acceptance Criterion · A successor that is another decision of the document raises no such error.**

###### RQ-183 · If the successors of superseded decisions form a cycle, then the validator shall report an error naming each decision in the cycle.

**AC-732 · Acceptance Criterion · Three decisions that supersede one another in a ring yield one error naming all three.**

**AC-733 · Acceptance Criterion · A chain of successors that ends at a decision that is not superseded raises no such error.**

###### RQ-184 · Where a decision is accepted, rejected, superseded or retired, the validator shall require at least two options, a driver and an assessment for each option and driver.

**AC-734 · Acceptance Criterion · Such a decision with fewer than two options is an error naming the decision.**

**AC-735 · Acceptance Criterion · Such a decision with no driver is an error naming the decision.**

**AC-736 · Acceptance Criterion · Such a decision without an assessment for an option and driver pair is an error naming the decision, the option and the driver.**

**AC-737 · Acceptance Criterion · A proposed decision raises none of these errors.**

**AC-738 · Acceptance Criterion · A decision with two options, a driver and every assessment raises no such error.**

###### RQ-185 · Where a decision is accepted, superseded or retired, the validator shall require a consequence and an `outcome` subject.

**AC-739 · Acceptance Criterion · Such a decision without a consequence is an error naming the decision.**

**AC-740 · Acceptance Criterion · Such a decision without a `subject` edge of role `outcome` is an error naming the decision.**

**AC-741 · Acceptance Criterion · A rejected or proposed decision raises none of these errors.**

**AC-742 · Acceptance Criterion · A decision with a consequence and an `outcome` subject raises no such error.**

#### FG-6 · Projections

The `spec/` tree is the specification, and the lock pins it. A projection is a derived, readable view of that tree that no role reads and no publish writes.

##### F-24 · Markdown projection

The Markdown view of a document for human readers: its heading levels, criterion paragraphs, trailing sections and the header naming the state root it came from.

###### RQ-107 · The renderer shall render the document as readable Markdown.

**AC-429 · Acceptance Criterion · A Product renders as a level-3 heading `<label> · <name>`.**

**AC-430 · Acceptance Criterion · A Feature Group renders as a level-4 heading.**

**AC-431 · Acceptance Criterion · A criterion renders as a bold paragraph `<label> · Acceptance Criterion · <text>`.**

**AC-432 · Acceptance Criterion · Components, decisions, the tag registry and `realizes` edges render in their own sections after the Products.**

**AC-433 · Acceptance Criterion · An unlabelled node renders with its key in place of a label.**

**AC-686 · Acceptance Criterion · Every node of the document appears in the rendering.**

**AC-687 · Acceptance Criterion · A Feature renders as a level-5 heading.**

**AC-688 · Acceptance Criterion · A Requirement renders as a level-6 heading with its statement text.**

**AC-875 · Acceptance Criterion · A node's tags render on one line after its body.**

**AC-876 · Acceptance Criterion · The rendering's title is the first Product's name.**

**AC-877 · Acceptance Criterion · Root Components render in the order the reader takes them.**

**AC-889 · Acceptance Criterion · A Component with a `boundary` field renders a line naming its value.**

###### RQ-108 · The renderer shall render deterministically.

**AC-434 · Acceptance Criterion · Two renders of one document are byte-identical.**

**AC-435 · Acceptance Criterion · The rendering opens with a header naming the state root it was rendered from.**

**AC-436 · Acceptance Criterion · The rendering contains no other digest.**

##### F-25 · Canonical JSON export

The machine-readable view of a document: every node's content view with its key and label, written as RFC 8785 canonical JSON.

**FT-16 · Facet Tag · portable**

###### RQ-109 · The renderer shall export the document as canonical JSON of its content view.

**AC-437 · Acceptance Criterion · The export is RFC 8785 canonical JSON.**

**AC-438 · Acceptance Criterion · The export holds every node's content view.**

**AC-439 · Acceptance Criterion · The export holds each node's key and label beside its content view.**

**AC-440 · Acceptance Criterion · The export names the state root it was rendered from.**

**AC-441 · Acceptance Criterion · Two exports of one document are byte-identical.**

**AC-655 · Acceptance Criterion · The export's top-level layout is the one the Format publishes with its golden vectors.**

###### RQ-169 · The renderer shall include the edges, the decisions and the registries in the JSON export.

**AC-652 · Acceptance Criterion · The export holds every edge with its type, its endpoints' keys and its role.**

**AC-653 · Acceptance Criterion · The export holds every decision's view with its key and label.**

**AC-654 · Acceptance Criterion · The export holds the tag registry and the constraint categories.**

##### F-26 · Projections are never authoritative

The guarantees that keep a projection derived: a render leaves `spec/` untouched, and the reader takes nothing from a projection.

###### RQ-110 · The renderer shall write nothing under `spec/`.

**AC-442 · Acceptance Criterion · A render leaves every file under `spec/` unchanged.**

**AC-443 · Acceptance Criterion · A render writes only to the paths it is given.**

###### RQ-111 · The reader shall take no input from a projection.

**AC-444 · Acceptance Criterion · A Markdown or JSON projection present in the repository changes no root.**

**AC-445 · Acceptance Criterion · Deleting every projection changes no root.**

**AC-446 · Acceptance Criterion · Editing a projection changes no root.**

**AC-689 · Acceptance Criterion · Editing a projection raises no finding.**

###### RQ-170 · If an output path lies under `spec/`, then the renderer shall refuse to render.

**AC-656 · Acceptance Criterion · The refusal names the path.**

**AC-657 · Acceptance Criterion · No projection is written.**

#### FG-7 · Publication

A publish pins a validated, committed `spec/` tree in the lock, and a later verification recomputes that lock's roots from the tree.

**FT-17 · Facet Tag · git**

##### F-27 · Preconditions

The three conditions that refuse a publish before it writes anything: uncommitted changes under `spec/`, a branch other than the publish branch, and a validation error.

###### RQ-112 · If the working tree has uncommitted changes under `spec/`, then the publisher shall refuse to publish.

**AC-447 · Acceptance Criterion · The refusal names the modified paths.**

**AC-448 · Acceptance Criterion · No lock is written.**

###### RQ-113 · If the current branch is not the publish branch, then the publisher shall refuse to publish.

**AC-449 · Acceptance Criterion · The publish branch is `main` unless the manifest names another.**

**AC-450 · Acceptance Criterion · The refusal names the current branch and the publish branch.**

**AC-690 · Acceptance Criterion · A detached HEAD is refused as not being on the publish branch.**

###### RQ-114 · If the validator reports an error, then the publisher shall refuse to publish.

**AC-451 · Acceptance Criterion · The refusal lists the errors.**

**AC-452 · Acceptance Criterion · No lock is written.**

**AC-453 · Acceptance Criterion · A lint finding does not refuse a publish.**

##### F-28 · Label allocation at publish

How a publish gives each unlabelled node the next label of its kind prefix while advancing the counters in `spec/labels.yaml`.

###### RQ-115 · When publishing, the publisher shall allocate a label to every node without one.

**AC-454 · Acceptance Criterion · Every node has a binding after a publish.**

**AC-455 · Acceptance Criterion · Each new label is the next value of the counter for the node's kind prefix.**

**AC-456 · Acceptance Criterion · Each counter in `spec/labels.yaml` advances to the next unallocated value.**

**AC-457 · Acceptance Criterion · The publisher reports each allocated label with its key.**

**AC-691 · Acceptance Criterion · Every decision has a binding after a publish.**

**AC-722 · Acceptance Criterion · No label that a decision carries as a former label is allocated.**

##### F-29 · The lock

The lock record a publish writes to `loft.lock.json`, together with the label changes that are the only edit a publish makes under `spec/`.

###### RQ-116 · When publishing a tree that changed since the last publish, the publisher shall write only the lock and the bindings file.

**AC-458 · Acceptance Criterion · `loft.lock.json` is written with the fields of the lock record.**

**AC-459 · Acceptance Criterion · No projection is written by a publish.**

**AC-460 · Acceptance Criterion · `spec/labels.yaml` is the only file under `spec/` that a publish changes.**

**AC-461 · Acceptance Criterion · The publish sequence is one more than the previous lock's, or 1 for a first publish.**

**AC-462 · Acceptance Criterion · `publishedAt` is the publish time in UTC.**

**AC-463 · Acceptance Criterion · If writing the lock or the bindings fails, neither is left changed.**

##### F-30 · Idempotence

A publish over a tree unchanged since the last one is a successful no-op that leaves the publish sequence where it was.

###### RQ-117 · While nothing has changed since the last publish, the publisher shall write nothing.

**AC-464 · Acceptance Criterion · A second publish over an unchanged tree changes no file.**

**AC-465 · Acceptance Criterion · A second publish over an unchanged tree reports success.**

**AC-466 · Acceptance Criterion · The publish sequence does not advance.**

**AC-640 · Acceptance Criterion · A tree has not changed when its recomputed state root equals the state root of the last lock.**

**AC-641 · Acceptance Criterion · An uncommitted change under `spec/` refuses the publish even when nothing has changed since the last publish.**

##### F-31 · Verification

A read-only check that the roots recomputed from `spec/` match the roots a lock records.

###### RQ-118 · The publisher shall verify a lock by recomputing the roots from `spec/`.

**AC-467 · Acceptance Criterion · Verification fails when the recomputed content root differs from the lock's.**

**AC-468 · Acceptance Criterion · Verification passes only when the recomputed state root equals the lock's.**

**AC-469 · Acceptance Criterion · Verification fails naming the first differing root.**

**AC-470 · Acceptance Criterion · Verification writes nothing.**

**AC-678 · Acceptance Criterion · The roots are compared in the order content root, binding root, state root.**

#### FG-8 · Merge semantics

How a merge driver combines two sides' edits to a `spec/` tree record by record, writing a typed conflict record wherever they disagree.

**FT-18 · Facet Tag · git**

##### F-32 · Three-way merge over keys

How the merge driver matches records across base, ours and theirs by key and combines the field changes of the two sides.

###### RQ-119 · The merge driver shall merge records by key.

**AC-471 · Acceptance Criterion · A record is matched across base, ours and theirs by its key.**

**AC-472 · Acceptance Criterion · A record present on one side only is kept.**

**AC-473 · Acceptance Criterion · A record deleted on one side and unchanged on the other is deleted.**

**AC-660 · Acceptance Criterion · A record deleted on one side and changed on the other becomes a conflict record with `null` for the deleting side.**

**AC-661 · Acceptance Criterion · A mapping entry without a key is matched across base, ours and theirs by its field name.**

**AC-662 · Acceptance Criterion · A list element without a key is matched across base, ours and theirs by its value.**

**AC-697 · Acceptance Criterion · The merged file is in the formatter's canonical layout.**

###### RQ-120 · When one side changes a field and the other leaves it, the merge driver shall take the change.

**AC-474 · Acceptance Criterion · The changed value is in the result.**

**AC-475 · Acceptance Criterion · No conflict record is written for the field.**

###### RQ-121 · When both sides change a field to the same value, the merge driver shall take that value once.

**AC-476 · Acceptance Criterion · The value appears once in the result.**

**AC-477 · Acceptance Criterion · No conflict record is written for the field.**

###### RQ-122 · If both sides change one field to different values, then the merge driver shall write a conflict record for that field.

**AC-478 · Acceptance Criterion · The record that owns the field gains a conflict record for it.**

**AC-479 · Acceptance Criterion · The other fields of the record are merged.**

**AC-855 · Acceptance Criterion · The field keeps its base value.**

##### F-33 · Ordered lists

How the merge driver combines insertions, deletions and moves in one ordered list while keeping base order.

###### RQ-123 · When both sides insert into one ordered list, the merge driver shall keep both insertions in base order.

**AC-480 · Acceptance Criterion · Each side's insertion follows its base predecessor.**

**AC-481 · Acceptance Criterion · No existing element is reordered.**

**AC-482 · Acceptance Criterion · An element deleted on one side and unmoved on the other is removed.**

**AC-663 · Acceptance Criterion · When both sides insert after one base element, our insertions precede theirs.**

###### RQ-124 · When one side moves a record and the other edits it, the merge driver shall apply both.

**AC-483 · Acceptance Criterion · The record is at the moved position.**

**AC-484 · Acceptance Criterion · The record carries the edit.**

**AC-696 · Acceptance Criterion · When both sides move one record to different positions, the merge driver writes a conflict record with field `position` on the moved record.**

**AC-856 · Acceptance Criterion · The moved record stays at its base position.**

##### F-34 · The typed conflict record

The record the merge driver adds to the record that owns a field the two sides changed to different values, with the validation error it holds open until removed.

###### RQ-125 · The merge driver shall write a conflict as a typed record in the file.

**AC-485 · Acceptance Criterion · The record holds `field`, `base`, `ours` and `theirs`.**

**AC-486 · Acceptance Criterion · The record sits in the `conflicts` list of the record that owns the field.**

**AC-487 · Acceptance Criterion · The driver exits non-zero when it wrote any conflict record.**

**AC-488 · Acceptance Criterion · The driver exits zero when it wrote none.**

###### RQ-126 · While a conflict record exists, the validator shall report an error.

**AC-489 · Acceptance Criterion · The error names the file and the key.**

**AC-490 · Acceptance Criterion · Removing the record clears the error.**

###### RQ-172 · The reader shall accept a `conflicts` list on any keyed record.

**AC-664 · Acceptance Criterion · A keyed record with a `conflicts` list is read without a read error.**

**AC-665 · Acceptance Criterion · A conflict record outside a `conflicts` list is an error naming the file, line and column.**

##### F-35 · The six scenarios

The six parallel-work cases, from edits to different Features to concurrent node creation, whose merged result the Format defines.

###### RQ-162 · When two sides edit different Features, the merge driver shall merge both edits without a conflict record.

**AC-618 · Acceptance Criterion · The merged tree carries both edits.**

**AC-619 · Acceptance Criterion · The merged tree validates.**

###### RQ-163 · When two sides append to one Feature, the merge driver shall keep both appends in base order.

**AC-620 · Acceptance Criterion · Both appended Requirements follow the Feature's base Requirements.**

**AC-621 · Acceptance Criterion · The merged tree validates.**

###### RQ-164 · When one side edits a Requirement and the other reorders it within its Feature, the merge driver shall keep the edit at the new position.

**AC-622 · Acceptance Criterion · The Requirement is at the reordered position.**

**AC-623 · Acceptance Criterion · The Requirement carries the edit.**

###### RQ-165 · When one side deletes a node and the other adds a reference to it, the merge driver shall merge both changes.

**AC-624 · Acceptance Criterion · The deleted node is absent from the merged tree.**

**AC-625 · Acceptance Criterion · The validator reports the dangling key.**

###### RQ-166 · When two sides edit one statement to different texts, the merge driver shall write a conflict record for the statement.

**AC-626 · Acceptance Criterion · The validator reports an error on the merged tree.**

**AC-627 · Acceptance Criterion · Removing the conflict record clears the error.**

###### RQ-167 · When two sides create nodes, the merge driver shall keep both nodes.

**AC-628 · Acceptance Criterion · The two created nodes have different keys.**

**AC-629 · Acceptance Criterion · Neither created node has a label.**

### P-2 · Loft CLI

The reference implementation's command line, `loft`: its commands, arguments, exit codes and JSON output. It fills every role the Format defines and adds only invocation.

**FT-14 · Facet Tag · git**

#### FG-9 · Commands

The `loft` commands that act on a `spec/` tree; each command that reads the tree states its refusal on a reader error as its own Requirement, so that refusal is tested per command.

**FT-19 · Facet Tag · git**

##### F-36 · `loft init`

Starts a new document: a `spec/` tree with a manifest, label counters at their first values and empty registries.

###### RQ-128 · `loft init` shall create an empty document in the current directory.

**AC-497 · Acceptance Criterion · `spec/` is created with a manifest.**

**AC-498 · Acceptance Criterion · The manifest carries a fresh document key.**

**AC-499 · Acceptance Criterion · `spec/labels.yaml` is created with every counter at its first value.**

**AC-500 · Acceptance Criterion · The registries are created empty.**

**AC-501 · Acceptance Criterion · The created tree is read back by the reader without error.**

**AC-502 · Acceptance Criterion · The created tree yields no validation error.**

**AC-503 · Acceptance Criterion · The created tree yields no lint finding.**

###### RQ-129 · If `spec/` already exists, then `loft init` shall refuse and exit 1.

**AC-504 · Acceptance Criterion · Nothing is written.**

**AC-505 · Acceptance Criterion · The error names the existing path.**

##### F-37 · `loft import v1`

Turns a Loft v1 export into a `spec/` tree through the converter, taking the label counters from a file or from the labels the export holds.

###### RQ-130 · `loft import v1 <export>` shall convert a Loft v1 export into `spec/`.

**AC-506 · Acceptance Criterion · Apart from the label counters, the tree equals the converter's output in canonical layout.**

**AC-508 · Acceptance Criterion · The sidecar is written to `v1-sidecar.json` beside `spec/`.**

**AC-509 · Acceptance Criterion · The conversion findings are printed as a lint report.**

###### RQ-131 · Where `--counters <file>` is given, `loft import v1` shall take the label counters from it.

**AC-510 · Acceptance Criterion · The file is a YAML mapping of label prefix to next value.**

**AC-511 · Acceptance Criterion · Each prefix named in the file sets that prefix's counter.**

**AC-512 · Acceptance Criterion · A prefix absent from the file is set to the highest value seen plus one.**

###### RQ-132 · If a counter in `--counters` is not above the highest label seen for its prefix, then `loft import v1` shall refuse and exit 1.

**AC-513 · Acceptance Criterion · The error names the prefix, the file's value and the highest label seen.**

**AC-514 · Acceptance Criterion · Nothing is written.**

###### RQ-133 · While `--counters` is absent, `loft import v1` shall set each counter to the highest value seen plus one.

**AC-515 · Acceptance Criterion · Each prefix's counter equals its highest label value plus one.**

**AC-516 · Acceptance Criterion · A warning names the prefixes and states that the source may have allocated higher values.**

###### RQ-134 · If `spec/` is not empty, then `loft import v1` shall refuse and exit 1.

**AC-517 · Acceptance Criterion · Nothing is written.**

**AC-518 · Acceptance Criterion · The error names the `spec/` path.**

###### RQ-135 · If `<export>` cannot be read as a Loft v1 export, then `loft import v1` shall refuse and exit 1.

**AC-519 · Acceptance Criterion · A missing file is reported by its path.**

**AC-520 · Acceptance Criterion · A file that is not a v1 export is reported by its path and the first missing field.**

**AC-521 · Acceptance Criterion · Nothing is written.**

###### RQ-173 · If the `--counters` file cannot be read as a mapping of label prefixes to positive integers, then `loft import v1` shall refuse and exit 1.

**AC-668 · Acceptance Criterion · A missing file is reported by its path.**

**AC-669 · Acceptance Criterion · A prefix outside the Format's kind prefixes is reported by name.**

**AC-670 · Acceptance Criterion · A value that is not a positive integer is reported with its prefix.**

**AC-671 · Acceptance Criterion · Nothing is written.**

##### F-38 · `loft check`

Runs the validator and then the linter over the tree, with an exit code a CI job can gate on.

**QA-1 · Quality Attribute · `loft check` shall complete within 2 seconds at the 95th percentile on a document of five thousand nodes.**

Quality target: `latency`; fate `sampled`; metric `check.wall-time`; aggregation `p95`; band `<= 2` `s`; scenario ```loft check` over a canonical tree of five thousand nodes with a warm page cache``; environment `the reference developer machine named in the repository`

###### RQ-136 · `loft check` shall run the validator and then the linter over the tree.

**AC-522 · Acceptance Criterion · Validation errors are printed before lint findings.**

**AC-523 · Acceptance Criterion · Every validation error is reported, not only the first.**

**AC-524 · Acceptance Criterion · `loft check` exits 1 when the validator reports an error.**

**AC-525 · Acceptance Criterion · Without `--fail-on`, `loft check` exits 0 when only lint findings exist.**

**AC-526 · Acceptance Criterion · Without `--format json`, `loft check` prints nothing on a clean tree.**

**AC-636 · Acceptance Criterion · `loft check` exits 0 on a clean tree.**

###### RQ-137 · If the reader reports an error, then `loft check` shall report it and exit 1.

**AC-527 · Acceptance Criterion · Every read error is reported with the position the reader names.**

**AC-528 · Acceptance Criterion · The linter does not run.**

**AC-865 · Acceptance Criterion · Within one file, the reader reports the first error it meets.**

###### RQ-138 · If there is no `spec/` directory, then `loft check` shall refuse and exit 1.

**AC-529 · Acceptance Criterion · The error names the expected path.**

###### RQ-139 · Where `--fail-on <severity>` is given, `loft check` shall exit 1 on a finding at or above that severity.

**AC-530 · Acceptance Criterion · `--fail-on warning` exits 1 on a warning.**

**AC-531 · Acceptance Criterion · `--fail-on info` exits 1 on an info finding.**

**AC-532 · Acceptance Criterion · `--fail-on hint` exits 1 on any finding.**

**AC-533 · Acceptance Criterion · A severity outside warning, info and hint is a usage error.**

**AC-638 · Acceptance Criterion · `--fail-on info` exits 1 on a warning.**

**AC-639 · Acceptance Criterion · `--fail-on warning` exits 0 when only info and hint findings exist.**

##### F-39 · `loft fmt`

Rewrites the tree in canonical layout and mints missing keys, or with `--check` reports drift without writing.

**QA-2 · Quality Attribute · `loft fmt --check` shall complete within 2 seconds at the 95th percentile on a canonical document of five thousand nodes.**

Quality target: `latency`; fate `sampled`; metric `fmt.check.wall-time`; aggregation `p95`; band `<= 2` `s`; scenario ```loft fmt --check` over a canonical tree of five thousand nodes with a warm page cache``; environment `the reference developer machine named in the repository`

###### RQ-140 · `loft fmt` shall rewrite `spec/` in canonical layout.

**AC-534 · Acceptance Criterion · Every file whose bytes would change is rewritten in canonical layout.**

**AC-535 · Acceptance Criterion · A file whose bytes would not change is not rewritten.**

**AC-536 · Acceptance Criterion · A key is minted for every node without one.**

**AC-537 · Acceptance Criterion · Files are written through a temporary directory and a rename.**

**AC-538 · Acceptance Criterion · The paths rewritten are printed.**

###### RQ-141 · If the reader reports an error, then `loft fmt` shall write nothing and exit 1.

**AC-539 · Acceptance Criterion · Every read error is reported with the position the reader names.**

**AC-540 · Acceptance Criterion · No file is changed.**

###### RQ-142 · Where `--check` is given, `loft fmt` shall report drift without writing.

**AC-541 · Acceptance Criterion · The drifting paths are printed.**

**AC-542 · Acceptance Criterion · A node without a key counts as drift.**

**AC-543 · Acceptance Criterion · No file is changed.**

**AC-544 · Acceptance Criterion · `--check` exits 0 on a canonical tree.**

**AC-637 · Acceptance Criterion · `--check` exits 1 when any file drifts.**

###### RQ-219 · When `loft fmt` mints the key of a Product or Feature file, `loft fmt` shall add the key to the list that names the file.

**AC-850 · Acceptance Criterion · A new Product file's key is appended to the manifest's `products`.**

**AC-851 · Acceptance Criterion · A new Feature file's key is appended to the `features` of the Feature Group whose directory holds the file.**

**AC-852 · Acceptance Criterion · An appended key follows the keys already listed.**

**AC-853 · Acceptance Criterion · A new Feature file that no Feature Group's directory holds is reported as unlisted.**

**AC-854 · Acceptance Criterion · When a new file is reported as unlisted, `loft fmt` changes no file.**

##### F-40 · `loft render`

Writes the Markdown projection and the canonical JSON export of the tree, to the paths given or to their defaults under `docs/`.

###### RQ-143 · `loft render` shall write the Markdown and JSON projections.

**AC-545 · Acceptance Criterion · `--markdown <path>` writes the Markdown projection to that path.**

**AC-546 · Acceptance Criterion · `--json <path>` writes the canonical JSON export to that path.**

**AC-547 · Acceptance Criterion · Without an output option, both are written to `docs/spec.md` and `docs/spec.json`.**

**AC-548 · Acceptance Criterion · A projection file whose bytes would not change is not rewritten.**

**AC-549 · Acceptance Criterion · Lint findings do not prevent a render.**

**AC-684 · Acceptance Criterion · With only one of `--markdown` and `--json`, only that projection is written.**

**AC-685 · Acceptance Criterion · A validation error does not prevent a render, except a successor that leads back to its own decision.**

###### RQ-144 · If the reader reports an error, then `loft render` shall write nothing and exit 1.

**AC-550 · Acceptance Criterion · Every read error is reported with the position the reader names.**

**AC-551 · Acceptance Criterion · No projection is written.**

###### RQ-171 · If an output path lies under `spec/`, then `loft render` shall refuse and exit 1.

**AC-658 · Acceptance Criterion · The error names the path.**

**AC-659 · Acceptance Criterion · No projection is written.**

##### F-41 · `loft publish`

Runs the publisher over the tree inside a git repository and prints what it allocated and the new state root, or why it refused.

###### RQ-145 · `loft publish` shall run the publisher and report its outcome.

**AC-552 · Acceptance Criterion · A refusal is printed with its reason and exits 1.**

**AC-553 · Acceptance Criterion · A publish prints each allocated label with its key.**

**AC-554 · Acceptance Criterion · A publish prints the new publish sequence and the state root and exits 0.**

**AC-555 · Acceptance Criterion · A publish with nothing changed prints that nothing changed and exits 0.**

###### RQ-146 · If the reader reports an error, then `loft publish` shall write nothing and exit 1.

**AC-556 · Acceptance Criterion · Every read error is reported with the position the reader names.**

**AC-557 · Acceptance Criterion · No file is changed.**

###### RQ-147 · If the working directory is not inside a git repository, then `loft publish` shall refuse and exit 1.

**AC-558 · Acceptance Criterion · The error states that a git repository is required.**

**AC-559 · Acceptance Criterion · Nothing is written.**

##### F-42 · `loft verify`

Recomputes the roots of the tree and compares them with the lock, with an exit code a CI job can gate on.

###### RQ-148 · `loft verify` shall compare the recomputed roots with the lock.

**AC-560 · Acceptance Criterion · On a match it prints the state root and exits 0.**

**AC-561 · Acceptance Criterion · On a mismatch it names the first differing root and exits 1.**

**AC-562 · Acceptance Criterion · Without a lock it exits 1 naming the missing file.**

**AC-667 · Acceptance Criterion · A lock that is not a valid lock record exits 1 naming the lock file.**

###### RQ-149 · If the reader reports an error, then `loft verify` shall report it and exit 1.

**AC-563 · Acceptance Criterion · Every read error is reported with the position the reader names.**

**AC-564 · Acceptance Criterion · No root is compared.**

##### F-43 · `loft merge-driver`

Runs the merge engine as the git merge driver for `spec/` files, and with `--install` registers that driver in the repository.

###### RQ-150 · `loft merge-driver <base> <ours> <theirs> <path>` shall run the merge engine as a git merge driver.

**AC-565 · Acceptance Criterion · The merged content is written to `<ours>`.**

**AC-566 · Acceptance Criterion · Messages name `<path>`, not the temporary files.**

**AC-567 · Acceptance Criterion · It exits 0 when no conflict record was written.**

**AC-568 · Acceptance Criterion · It exits 1 when a conflict record was written.**

###### RQ-151 · If any of `<base>`, `<ours>` or `<theirs>` cannot be read as the Format's YAML subset, then `loft merge-driver` shall write nothing and exit 1.

**AC-569 · Acceptance Criterion · The error names which input failed and where.**

**AC-570 · Acceptance Criterion · `<ours>` is unchanged.**

###### RQ-152 · `loft merge-driver --install` shall register the driver in the repository.

**AC-571 · Acceptance Criterion · `.gitattributes` gains `spec/\*\*/\*.yaml merge=loft` when absent.**

**AC-572 · Acceptance Criterion · `merge.loft.driver` is set to `loft merge-driver %O %A %B %P` in the repository's git configuration.**

**AC-573 · Acceptance Criterion · `merge.loft.name` is set to a description of the driver.**

**AC-574 · Acceptance Criterion · A second `--install` changes nothing.**

###### RQ-153 · If the working directory is not inside a git repository, then `loft merge-driver --install` shall refuse and exit 1.

**AC-575 · Acceptance Criterion · The error states that a git repository is required.**

**AC-576 · Acceptance Criterion · Nothing is written.**

#### FG-10 · Invocation

The conventions that every `loft` command shares: exit codes, JSON output, the version line and error positions.

##### F-44 · Exit codes

Separates success, a failed operation and a mistake in the invocation, so that a script can branch on the exit code alone.

###### RQ-154 · `loft` shall exit 0 on success, 1 on an error and 2 on a usage error.

**AC-577 · Acceptance Criterion · A completed command exits 0.**

**AC-578 · Acceptance Criterion · A refusal, a failed check or a failed verification exits 1.**

**AC-579 · Acceptance Criterion · A failure the command does not expect exits 1 with its message on standard error.**

**AC-580 · Acceptance Criterion · An unknown command, an unknown option or a missing argument exits 2.**

**AC-581 · Acceptance Criterion · `loft` with no command prints the usage and exits 2.**

**AC-683 · Acceptance Criterion · `loft --help` prints the usage and exits 0.**

##### F-45 · JSON output

Gives scripts and agents a machine-readable result for each command, described by a versioned schema per command under `schemas/`.

###### RQ-155 · Where `--format json` is given, `loft` shall print one JSON document on standard output.

**AC-582 · Acceptance Criterion · The document matches the command's result schema.**

**AC-583 · Acceptance Criterion · An error is reported as the JSON document on standard output.**

**AC-584 · Acceptance Criterion · Human-readable text goes to standard error only.**

**AC-585 · Acceptance Criterion · The exit code is the same as without `--format json`.**

###### RQ-156 · `loft` shall describe each command's JSON result by a schema published under `schemas/`.

**AC-586 · Acceptance Criterion · Each command has one schema file named after it.**

**AC-587 · Acceptance Criterion · Each schema names its version.**

**AC-588 · Acceptance Criterion · A change to a command's output changes its schema file.**

##### F-46 · Version

Reports the tool release together with the format and the hashing algorithm it implements, on three fixed lines.

###### RQ-157 · `loft --version` shall print the tool version, the format identifier and the algorithm identifier.

**AC-589 · Acceptance Criterion · The first line is `loft <semver>`.**

**AC-590 · Acceptance Criterion · The second line is `format loftspec/1`.**

**AC-591 · Acceptance Criterion · The third line is `algorithm sha256-jcs/1`.**

**AC-682 · Acceptance Criterion · Under `--format json`, the three values are printed as one JSON document.**

##### F-47 · Error positions

Reports errors in the `path:line:column` form that editors and CI annotations can jump to.

###### RQ-158 · When an error concerns a position in a file, `loft` shall name the file, line and column.

**AC-592 · Acceptance Criterion · A text error reads `<path>:<line>:<column>: <message>`.**

**AC-593 · Acceptance Criterion · A JSON error carries `path`, `line` and `column`.**

**AC-594 · Acceptance Criterion · An error without a file position omits them.**

**AC-595 · Acceptance Criterion · Each text error is reported on its own line.**

**AC-679 · Acceptance Criterion · An error that concerns two positions names each as `<path>:<line>:<column>`.**

**AC-680 · Acceptance Criterion · Lines and columns are counted from 1.**

**AC-681 · Acceptance Criterion · A column counts Unicode scalar values.**

## Component Model

### CMP-1 · loftspec

Scope: Product `P-1`

The Rust library crate implementing the Format: document model, identity, canonical bytes, hashing, conversion, validation, lint, rendering, publication and merging. It performs no input or output.

**CON-1 · Constraint · `loftspec` shall perform no file or network input or output.**

**CON-2 · Constraint · `loftspec` shall declare the Apache-2.0 licence in its package manifest.**

**CON-3 · Constraint · `loftspec` shall build on stable Rust at the crate's declared `rust-version`.**

#### CMP-2 · Document model

Nodes, keys, labels, bindings, statements and sections.

**CON-4 · Constraint · The document model shall reject a node whose kind is outside the Format's closed set of kinds.**

#### CMP-3 · Hasher

Content views, digests, roots and the state view.

**CON-5 · Constraint · The hasher shall use SHA-256 as its only digest function.**

#### CMP-4 · Reader and writer

The text form: the `spec/` tree read into a document and written back in canonical layout. The writer yields paths and bytes; writing them to disk belongs to a store outside this crate.

**CON-6 · Constraint · The reader and writer shall read and write UTF-8 only.**

#### CMP-5 · Converter

Conversion from a Loft v1 export, with the sidecar.

**CON-7 · Constraint · The converter shall take a v1 export file as its only input.**

**FT-13 · Facet Tag · conversion**

#### CMP-6 · Validator

Containment and reference checks that decide whether a document is valid.

**CON-8 · Constraint · The validator shall change nothing in the document it validates.**

#### CMP-7 · Linter

Structural, language and v2 lint rules, their findings and reports, and the lint configuration kept in the repository. The linter takes `spec/lint.yaml` as part of the tree it is given and reads no file itself.

**CON-9 · Constraint · The linter shall change nothing in the document it lints.**

#### CMP-8 · Renderer

The Markdown projection and the canonical JSON export, each derived from the `spec/` tree. The renderer yields their bytes; writing them to the given paths belongs to a store outside this crate.

**CON-10 · Constraint · The renderer shall depend on no clock, locale or environment variable.**

#### CMP-9 · Publisher

Publication preconditions, label allocation, the lock and its verification. The publisher decides from the tree, the repository state and the previous lock it is given, and yields the lock and the bindings to write; reading the repository and writing files belong to a store outside this crate.

**CON-11 · Constraint · The publisher shall compute every root it writes with the hasher.**

#### CMP-10 · Merge engine

The merge driver's three-way merge over keys and ordered lists, with the typed conflict record.

**CON-12 · Constraint · The merge engine shall take as input only the base, ours and theirs versions of one file.**

### CMP-11 · loft-cli

Scope: Product `P-2`

The `loft` binary.

**CON-13 · Constraint · `loft-cli` shall expose exactly one binary, named `loft`.**

### CMP-12 · loftspec-store

Scope: Product `P-2`

Memory and file stores for documents.

**CON-14 · Constraint · `loftspec-store` shall write a `spec/` tree through a temporary directory and a rename.**

## Architecture Decisions

### Active Architecture Decisions

#### ADR-1 · Keys are UUIDs, version 7 for new keys

Status: `accepted`

##### Context

Every node carries its identity in the file, and a clone preserves it. Nodes converted from a Loft v1 export keep their v1 UUIDs; new nodes need keys minted locally by the formatter, with no server. The key syntax decides whether a document carries one key shape or two, which dependency every implementer must adopt, and whether keys sort by the time they were minted.

##### Drivers

1. **One key shape** — Converted and new keys share one syntax, so the reader, the validator and the merge driver treat every key alike.
2. **Minimal dependencies** — The library adds no dependency that a second implementer must also adopt.
3. **Time-ordered minting** — A key minted later sorts after a key minted earlier, so appended nodes keep key order and file order aligned.

##### Options

1. **UUID version 7** — New keys are RFC 9562 version 7 UUIDs. Converted nodes keep their UUIDs. One syntax, one dependency, time-ordered minting.
2. **ULID** — New keys are ULIDs, 26 characters of Crockford base32. Converted nodes keep their UUIDs, so two syntaxes coexist in one document and a second dependency is needed.
3. **Prefixed short identifiers** — Short random identifiers with a kind prefix, such as `rq\_8f3k2`. Readable, but a bespoke syntax with its own collision budget, beside the converted UUIDs.

##### Assessment

| Option | One key shape | Minimal dependencies | Time-ordered minting |
| --- | --- | --- | --- |
| UUID version 7 | supports — the same RFC 9562 syntax as converted keys | supports — `uuid` is already required for converted keys | supports — version 7 embeds a millisecond timestamp |
| ULID | conflicts — two syntaxes in one document for ever | conflicts — a ULID crate beside `uuid` | supports — ULIDs are time-ordered |
| Prefixed short identifiers | conflicts — two syntaxes in one document | neutral — no crate, but a bespoke generator and parser | conflicts — random identifiers do not sort by time |

##### Decision

Selected: **UUID version 7**

New keys are UUID version 7; a key of any UUID version read from a file is kept; the formatter mints keys; `uuid` is the only identity dependency.

##### Consequences

- **benefit** — One validator rule covers every key in the document.
- **obligation** — The formatter mints version 7 keys for nodes without one. (`RQ-3` · When formatting a node without a key, the formatter shall mint a version 7 UUID as its key.)
- **risk** — A version 7 key reveals the time it was minted, which a specification can tolerate.

##### Subjects

- **affected** — `CMP-2` · Document model
- **outcome** — `RQ-3` · When formatting a node without a key, the formatter shall mint a version 7 UUID as its key.
- **outcome** — `RQ-1` · The writer shall write a key on every node.

#### ADR-2 · Edge endpoints are hashed by content, as a multiset

Status: `accepted`

##### Context

Two documents that say the same thing must have the same content root, on any implementation, so the content root cannot depend on keys. Edges join nodes. The question is how an edge names its endpoints inside the digest, and how two equal edges are counted.

##### Drivers

1. **Key-free content root** — The content root does not change when keys change.
2. **Duplicates stay honest** — Two nodes with identical content and identical edges both count, so a copied subtree is visible in the root.
3. **Stable under relabelling** — A publish that only assigns labels does not move the content root.

##### Options

1. **Content hashes, multiset** — Each edge enters the digest as the tuple of its source's content hash, its type, its target's content hash and its role; the edges root is the digest of the sorted multiset of tuples.
2. **Keys** — Each edge names its endpoints by key; the edges root depends on keys.
3. **Labels** — Each edge names its endpoints by label; the edges root depends on publication.

##### Assessment

| Option | Key-free content root | Duplicates stay honest | Stable under relabelling |
| --- | --- | --- | --- |
| Content hashes, multiset | supports — no key enters the tuple | supports — the multiset counts equal tuples twice | supports — labels never enter the tuple |
| Keys | conflicts — fresh keys change the root | supports — keys distinguish the duplicates | supports — labels not involved |
| Labels | conflicts — an unpublished node has no label, so the root cannot be computed before publish | supports — labels distinguish the duplicates | conflicts — a relabel changes the tuple |

##### Decision

Selected: **Content hashes, multiset**

An edge enters the edges root as the tuple of its endpoints' content hashes, its type and its role; the edges root is the digest of the sorted multiset of such tuples.

##### Consequences

- **benefit** — The content root is portable across instances and implementations.
- **obligation** — The hasher recomputes an edge's tuple whenever an endpoint's content changes. (`RQ-13` · The hasher shall compute the edges root over the sorted multiset of edges, each as its source's hash, its type, its target's hash and its role.)
- **risk** — Two content-identical nodes cannot be told apart inside the edges root; the bindings keep that distinction.

##### Subjects

- **affected** — `CMP-3` · Hasher
- **outcome** — `RQ-14` · The hasher shall compute the content root from the Product roots in document order, the Component roots sorted, the edges root, the registry roots and the decisions root.
- **outcome** — `RQ-13` · The hasher shall compute the edges root over the sorted multiset of edges, each as its source's hash, its type, its target's hash and its role.

#### ADR-3 · One file per Feature

Status: `accepted`

##### Context

The text form is a tree of YAML files held in git. The file granularity decides what a merge conflict touches, what a review diff shows and how many files a document has. A document of a thousand Requirements has about two hundred Features.

##### Drivers

1. **Merge granularity** — Concurrent edits to different units do not touch one file.
2. **Reviewable diffs** — A change shows as the diff of one unit with its context around it.
3. **Bounded file count** — The tree has hundreds of files, not thousands.

##### Options

1. **One file per Feature** — A Feature file holds the Feature, its Requirements and their criteria; the Feature Group lists its Features' keys in the Product file.
2. **One file per Product** — A whole Product, with every Feature Group, Feature, Requirement and criterion, in one file.
3. **One file per Requirement** — Each Requirement with its criteria in its own file; Features list Requirement keys.

##### Assessment

| Option | Merge granularity | Reviewable diffs | Bounded file count |
| --- | --- | --- | --- |
| One file per Feature | supports — most parallel edits land in different files | supports — a Requirement change shows inside its Feature | supports — about two hundred files for a large document |
| One file per Product | conflicts — every edit touches one large file | conflicts — a small change sits in a diff of thousands of lines | supports — a handful of files |
| One file per Requirement | supports — the finest granularity | neutral — a Requirement's context, its Feature, is in another file | conflicts — thousands of files |

##### Decision

Selected: **One file per Feature**

Each Feature is written to its own file with its Requirements and criteria; the Feature Group lists its Features' keys in order in the Product file.

##### Consequences

- **benefit** — Most parallel edits touch different files and merge without a driver.
- **obligation** — The reader checks that every listed key has a file and every Feature file is listed. (`RQ-28` · The reader shall take the order of Features from their Feature Group's key list.)
- **cost** — Moving a Requirement between Features is a two-file change.

##### Subjects

- **affected** — `CMP-4` · Reader and writer
- **outcome** — `RQ-28` · The reader shall take the order of Features from their Feature Group's key list.
- **outcome** — `RQ-22` · The writer shall write each Feature to its own file under its Product's directory.

#### ADR-4 · Labels are assigned at publish on the publish branch

Status: `accepted`

##### Context

Labels such as `RQ-23` are the human-readable handles of nodes. They must be unique and are never reused. In a git repository two people create nodes on separate branches with no shared counter. When a label is assigned decides whether parallel work can collide.

##### Drivers

1. **No serial step during authoring** — Two engineers create nodes on branches without coordinating.
2. **Never reused** — A label, once allocated, is never given to another node.
3. **Human-readable handles** — Labels stay short, sequential and speakable.

##### Options

1. **At publish on the publish branch** — Keys at creation; the publisher allocates labels from per-prefix counters that advance only at publish.
2. **At creation from a shared counter** — Every creation takes the next value of a counter, which needs a shared service or a locked file.
3. **Content-derived labels** — A label is derived from the node's content hash; no counter, but the label changes whenever the content does.

##### Assessment

| Option | No serial step during authoring | Never reused | Human-readable handles |
| --- | --- | --- | --- |
| At publish on the publish branch | supports — branches allocate nothing | supports — the counter only moves forward at publish | supports — sequential per prefix |
| At creation from a shared counter | conflicts — every creation needs the counter | supports — the counter never rewinds | supports — sequential per prefix |
| Content-derived labels | supports — no counter | conflicts — a label changes with the content, so a handle is not kept | conflicts — hash fragments are neither short nor speakable |

##### Decision

Selected: **At publish on the publish branch**

A node has no label until it is published; the publisher allocates labels from per-prefix counters that never rewind; an unpublished node is referenced by key and rendered by key.

##### Consequences

- **benefit** — Branches never conflict on labels.
- **cost** — Prose cannot cite a label for an unpublished node; a render shows the key instead.
- **obligation** — The publisher records a relabelled node's previous label as a permanent alias. (`RQ-9` · When a node's label changes, the publisher shall record the previous label as a permanent alias of the same key.)

##### Subjects

- **affected** — `CMP-2` · Document model
- **outcome** — `RQ-6` · The writer shall record labels only in the bindings file.
- **outcome** — `RQ-7` · The publisher shall allocate each label from a per-prefix counter.

#### ADR-5 · YAML is the text form

Status: `accepted`

##### Context

One text form, one canonicalisation, one versioned hash. The text form must round-trip bodies byte for byte, be written in one canonical layout by a formatter, and read well in a git diff. Three candidates were weighed: a YAML subset, Markdown with front matter, and JSON.

##### Drivers

1. **Exact body round trip** — Multi-line prose with trailing spaces, tabs and any Unicode survives a write and a read byte for byte.
2. **Canonical formatting** — One document has one byte sequence after formatting.
3. **Readable diffs** — A reviewer reads a change in a pull request.

##### Options

1. **A YAML subset with a canonical formatter** — Block mappings and sequences, plain, double-quoted and literal block scalars; the formatter fixes key order, indentation and scalar style.
2. **Markdown with YAML front matter** — One Markdown file per node or Feature; structure in the front matter, bodies as Markdown sections.
3. **JSON** — Canonical by RFC 8785 directly; no comments; prose as escaped strings.

##### Assessment

| Option | Exact body round trip | Canonical formatting | Readable diffs |
| --- | --- | --- | --- |
| A YAML subset with a canonical formatter | supports — literal block scalars, with double quotes where a block scalar would lie | supports — a fixed subset with a fixed key order | supports — indented text with prose in blocks |
| Markdown with YAML front matter | conflicts — a heading or fence inside a body collides with the file's own structure | neutral — front matter can be canonical; the Markdown body layout cannot be forced | supports — prose reads as prose |
| JSON | supports — strings are exact | supports — RFC 8785 | conflicts — prose becomes escaped single-line strings |

##### Decision

Selected: **A YAML subset with a canonical formatter**

The text form is the YAML subset the Format defines, written only by the formatter in canonical layout; the canonical bytes for hashing are the RFC 8785 JSON of the content view, never the YAML.

##### Consequences

- **benefit** — Bodies round-trip exactly and diffs read naturally.
- **cost** — A YAML parser for the subset must be chosen or written; the subset is small enough to parse directly.
- **obligation** — The reader rejects every construct outside the subset. (`RQ-33` · The reader shall accept only the YAML subset the Format defines.)

##### Subjects

- **affected** — `CMP-4` · Reader and writer
- **outcome** — `RQ-33` · The reader shall accept only the YAML subset the Format defines.
- **outcome** — `RQ-35` · The formatter shall write every string in the canonical scalar style.

#### ADR-6 · Bodies are stored verbatim on import

Status: `accepted`

##### Context

A Loft v1 export carries bodies with headings and other prose that the v2 model would express as Section nodes. The converter can transform such bodies during import or store them byte for byte and report them.

##### Drivers

1. **Exact round trip** — The source state's four addresses must be reproducible, so body bytes must be preserved.
2. **No silent change of meaning** — Import alters nothing the document says without an author's decision.
3. **A clean v2 outline** — Bodies should eventually carry no headings.

##### Options

1. **Verbatim, with findings** — Bodies are stored unchanged; a heading in a body is reported; authors restructure after import.
2. **Split headings into Sections automatically** — The converter turns each heading and the text beneath it into a Section node during import.

##### Assessment

| Option | Exact round trip | No silent change of meaning | A clean v2 outline |
| --- | --- | --- | --- |
| Verbatim, with findings | supports — bytes preserved | supports — nothing changes without an author | neutral — clean-up is deferred to authors, who are told where |
| Split headings into Sections automatically | conflicts — body bytes change | conflicts — a heuristic split changes structure | supports — no heading remains in a body |

##### Decision

Selected: **Verbatim, with findings**

The converter stores every body byte for byte and reports a body with headings as a finding; restructuring into Sections is an authored change after import.

##### Consequences

- **benefit** — The v1 round trip stays exact.
- **obligation** — The linter reports a heading inside a body so authors can restructure.
- **cost** — An imported document carries findings until its authors restructure the bodies.

##### Subjects

- **affected** — `CMP-5` · Converter
- **outcome** — `RQ-54` · The converter shall store bodies verbatim.

#### ADR-8 · A merge conflict is a typed record in the file

Status: `accepted`

##### Context

Two branches can change the same field of the same node. git's default is conflict markers, which make a YAML file unreadable to every tool until a person repairs it. The Format needs a conflict representation that keeps the tree readable, lets the validator find every conflict, and lets a tool present both sides.

##### Drivers

1. **The file stays readable** — A tree with a conflict still parses.
2. **The validator finds every conflict** — An unresolved conflict cannot be published by accident.
3. **Tools can present both sides** — A reviewer or an agent sees base, ours and theirs.

##### Options

1. **Typed conflict record** — The merge driver writes the field as a record holding base, ours and theirs; the validator reports every such record.
2. **git conflict markers** — The merge driver leaves `<<<<<<<` markers in the file.
3. **Refuse the merge** — The merge driver fails and leaves resolution to the person.

##### Assessment

| Option | The file stays readable | The validator finds every conflict | Tools can present both sides |
| --- | --- | --- | --- |
| Typed conflict record | supports — the record is part of the YAML subset | supports — the validator reports each record | supports — base, ours and theirs are fields |
| git conflict markers | conflicts — markers break the YAML | neutral — the reader fails, which stops a publish but names no field | conflicts — sides are text fragments, not fields |
| Refuse the merge | supports — nothing is written | conflicts — nothing is recorded to find | conflicts — nothing is recorded to present |

##### Decision

Selected: **Typed conflict record**

The merge driver writes each conflicting field as a typed conflict record holding the base, ours and theirs values; the validator reports an error while any record exists; the merge driver exits 1 when it wrote one.

##### Consequences

- **benefit** — A conflicted tree still reads, renders and diffs.
- **cost** — The YAML subset and the reader grow one record kind.
- **obligation** — The validator reports every conflict record. (`RQ-126` · While a conflict record exists, the validator shall report an error.)

##### Subjects

- **affected** — `CMP-10` · Merge engine
- **outcome** — `RQ-125` · The merge driver shall write a conflict as a typed record in the file.
- **outcome** — `RQ-126` · While a conflict record exists, the validator shall report an error.

#### ADR-9 · One invocation convention across commands

Status: `accepted`

##### Context

`loft` is driven by people, scripts and agents. If each command chooses its own exit codes, its own error shape and its own JSON layout, every caller learns eight conventions and misreads some of them.

##### Drivers

1. **One shape to parse** — Scripts and agents parse one result and one error shape.
2. **An exit code means one thing** — 0, 1 and 2 mean the same on every command.

##### Options

1. **One convention** — Every command shares exit codes 0, 1 and 2, one JSON document on standard output under `--format json`, and errors naming the file, line and column.
2. **Per-command conventions** — Each command chooses what fits it best.

##### Assessment

| Option | One shape to parse | An exit code means one thing |
| --- | --- | --- |
| One convention | supports — one error shape, one result envelope | supports — 0 success, 1 error, 2 usage everywhere |
| Per-command conventions | conflicts — a parser per command | conflicts — an exit code means what each command says |

##### Decision

Selected: **One convention**

Every `loft` command exits 0 on success, 1 on an error and 2 on a usage error; under `--format json` it prints one JSON document on standard output, errors included; an error about a file names the file, line and column.

##### Consequences

- **benefit** — One caller library serves every command.
- **cost** — A command with a natural third outcome expresses it in its JSON result, not its exit code.
- **obligation** — Each command's result schema is published under `schemas/`. (`RQ-156` · `loft` shall describe each command's JSON result by a schema published under `schemas/`.)

##### Subjects

- **affected** — `CMP-11` · loft-cli
- **outcome** — `RQ-155` · Where `--format json` is given, `loft` shall print one JSON document on standard output.
- **outcome** — `RQ-154` · `loft` shall exit 0 on success, 1 on an error and 2 on a usage error.

#### ADR-11 · Lint configuration lives in the repository

Status: `accepted`

##### Context

Lint findings gate a pull request. If configuration lives on each machine or only in command-line flags, two engineers, or an engineer and CI, see different findings for the same tree. The linter needs one place to read which rules run and at what severity, and that place should change only through review.

##### Drivers

1. **Same findings everywhere** — Two engineers and CI get the same findings for the same tree.
2. **Versioned with the spec** — A change of configuration is reviewed and merged like a change of the specification.
3. **Reproducible gate** — A CI run reproduces a local run without extra arguments.

##### Options

1. **`spec/lint.yaml`** — The configuration is a file of the tree, read by the linter, changed by commit.
2. **Configuration outside the repository** — Each user's configuration directory holds it.
3. **Command-line flags only** — Each invocation names the rules and severities it wants.

##### Assessment

| Option | Same findings everywhere | Versioned with the spec | Reproducible gate |
| --- | --- | --- | --- |
| `spec/lint.yaml` | supports — one file, read by every run | supports — it changes by commit and review | supports — CI reads the same file |
| Configuration outside the repository | conflicts — each machine has its own | conflicts — no history beside the spec | conflicts — CI has a configuration of its own |
| Command-line flags only | conflicts — each invocation may differ | conflicts — flags live in scripts, not the spec | neutral — a CI script can repeat the flags, if kept in step by hand |

##### Decision

Selected: **`spec/lint.yaml`**

The linter reads its configuration only from `spec/lint.yaml`; without that file it applies the default configuration; no flag or user file changes a finding.

##### Consequences

- **benefit** — Every run of the gate agrees.
- **cost** — A personal preference for a rule's severity needs a reviewed change.
- **obligation** — Without `spec/lint.yaml` the linter applies the default configuration. (`RQ-105` · The linter shall read its configuration from `spec/lint.yaml`.)

##### Subjects

- **affected** — `CMP-7` · Linter
- **outcome** — `RQ-105` · The linter shall read its configuration from `spec/lint.yaml`.
- **outcome** — `RQ-106` · The linter shall produce the same findings for the same tree and configuration on any machine.

#### ADR-12 · A publish writes no projection; projections are rendered on demand

Status: `accepted`

##### Context

A rendered Markdown and a JSON export committed beside the lock can be mistaken for the specification. In v2 the `spec/` tree is the specification. A second copy committed beside it can drift from it, and a reader that takes input from a projection would make the projection a source.

##### Drivers

1. **One source** — `spec/` is the only input any role reads.
2. **No second copy to drift** — Nothing committed restates the specification.
3. **Reproducible projections** — Any commit's Markdown or JSON can be rendered again exactly.

##### Options

1. **Lock and bindings only** — A publish writes the lock and the bindings; projections are rendered on demand and never committed by a publish.
2. **Lock and projections** — A publish also writes the Markdown and the JSON export beside the lock.
3. **Projections only** — A publish commits projections and no lock.

##### Assessment

| Option | One source | No second copy to drift | Reproducible projections |
| --- | --- | --- | --- |
| Lock and bindings only | supports — roles read only `spec/` | supports — nothing restates `spec/` | supports — rendering is deterministic from any commit |
| Lock and projections | neutral — `spec/` stays the input, but readers are invited to treat the Markdown as the contract | conflicts — two copies per publish | supports — the committed copy can be checked by rendering |
| Projections only | conflicts — without a lock nothing pins the roots | conflicts — the projection is the only record | neutral — reproducible, but nothing says which state it renders |

##### Decision

Selected: **Lock and bindings only**

A publish writes the lock and the bindings in `spec/labels.yaml` and nothing else; the Markdown and JSON projections are rendered on demand, each stamped with the state root it renders, and no role takes input from a projection.

##### Consequences

- **benefit** — The specification has one source.
- **cost** — A reader without the tool needs a rendering step or the hosted view.
- **obligation** — The reader takes no input from a projection. (`RQ-111` · The reader shall take no input from a projection.)

##### Subjects

- **affected** — `CMP-9` · Publisher
- **affected** — `CMP-8` · Renderer
- **outcome** — `RQ-116` · When publishing a tree that changed since the last publish, the publisher shall write only the lock and the bindings file.
- **outcome** — `RQ-111` · The reader shall take no input from a projection.

#### ADR-13 · A deleted node stays named by its former label

Status: `accepted`

##### Context

A retired or superseded decision keeps its history: the nodes it chose, affected and weighed. When such a node later leaves the document, the decision still names it, and Loft v1 exports keep those references. The Format names a node by key in files and by content hash in views, and neither exists for a node the document no longer holds.

##### Drivers

1. **Readable history** — A reader of a retired decision can tell which node it named.
2. **Keyless content** — The content root stays free of keys, as every other view is.
3. **Labels stay unique** — A label names one node for the life of the document.

##### Options

1. **Former label** — The reference is written as the label the node carried when it left the document.
2. **Null reference** — The reference stays, with `null` where the node would be named.
3. **Sidecar only** — The reference leaves the document, and only a conversion's sidecar keeps it.

##### Assessment

| Option | Readable history | Keyless content | Labels stay unique |
| --- | --- | --- | --- |
| Former label | supports — the label is the name readers used | supports — a label is not a key | supports — the validator refuses a former label an element carries, and the counters never reach it again |
| Null reference | conflicts — which node it was is lost | supports — `null` carries no key | neutral — no label is involved |
| Sidecar only | conflicts — the document loses the reference, and only a converted document has a sidecar | supports — nothing enters the content root | neutral — no label is involved |

##### Decision

Selected: **Former label**

A retired or superseded decision names a node the document no longer holds by that node's former label; the validator refuses a former label anywhere else and any former label an element carries, and the publisher allocates no former label again.

##### Consequences

- **benefit** — A retired decision stays readable after the nodes it named are deleted.
- **obligation** — The validator checks every former label against the live labels and aliases. (`RQ-180` · If a former label is the live label or an alias of an element, then the validator shall report an error naming the label and the element.)
- **cost** — Deleting a node that a retired or superseded decision names means rewriting that reference as the former label.

##### Subjects

- **affected** — `RQ-161` · The hasher shall build each decision view from the decision's content alone.
- **affected** — `RQ-51` · The converter shall turn each Architecture Decision into a decision with driver, option and consequence children, typed assessments and subject edges.
- **outcome** — `RQ-178` · Where a retired or superseded decision names a node the document no longer holds, the writer shall write that reference as the node's former label.

#### ADR-14 · The Format's own reader and writer implement its YAML subset

Status: `accepted`

##### Context

The Format admits a small YAML subset, rejects every construct outside it, a comment included, at a file, line and column, and fixes one canonical byte sequence for each document. The maintained YAML libraries for Rust discard comments while scanning and report nothing about them, none rejects the other excluded constructs on its own, and none writes the Format's canonical layout.

##### Drivers

1. **Exact rejection** — Every construct outside the subset, a comment included, is refused where it occurs.
2. **Positioned errors** — Every read error names a file, line and column.
3. **Canonical bytes** — The writer produces one byte sequence for each document, which a second implementation can reproduce.
4. **Few dependencies** — Reading and writing add no runtime dependency.

##### Options

1. **Own reader and writer** — The reader parses exactly the subset and refuses everything else as it scans; the writer emits the canonical layout directly.
2. **A YAML library and extra checks** — A conforming YAML library reads and writes, a second scanner finds the constructs the library accepts or discards, and a pass rewrites its output into the canonical layout.
3. **Comments allowed** — A YAML library reads and writes; comments are accepted and the formatter drops them.

##### Assessment

| Option | Exact rejection | Positioned errors | Canonical bytes | Few dependencies |
| --- | --- | --- | --- | --- |
| Own reader and writer | supports — each excluded construct is one branch of the scanner | supports — the scanner holds the position of every character | supports — the writer emits only the canonical layout | supports — no dependency |
| A YAML library and extra checks | neutral — exact only while the two scanners agree on every token | supports — both report positions | neutral — canonical only after a rewriting pass over the library's output | conflicts — a library dependency at version 0.1 |
| Comments allowed | conflicts — a comment is accepted, and the next format deletes it | supports — the library reports positions | neutral — canonical only after a rewriting pass over the library's output | conflicts — a library dependency at version 0.1 |

##### Decision

Selected: **Own reader and writer**

The Format's own reader parses the YAML subset and refuses each excluded construct at its position, its own writer emits the canonical layout, and a differential test holds both to a conforming YAML parser.

##### Consequences

- **cost** — The reader and writer are about fifteen hundred lines that the project maintains.
- **obligation** — A differential test compares the reader with a conforming YAML parser on every document the reader accepts, including every document the writer produces.
- **benefit** — An author learns of a comment or another excluded construct at its position, before any text is lost.

##### Subjects

- **affected** — `CMP-4` · Reader and writer
- **outcome** — `RQ-33` · The reader shall accept only the YAML subset the Format defines.
- **outcome** — `RQ-35` · The formatter shall write every string in the canonical scalar style.

#### ADR-15 · A Component's boundary is a kind that its subtree takes on

Status: `accepted`

##### Context

A Loft v1 Component carried one scope kind: a Product, the whole document, or external. The v2 text form keeps a Product scope as a `scoped_to` edge and nothing else. A document authored in v2 therefore cannot say that a Component is a service outside Loft's governance, and the obligation rules report every such Component.

##### Drivers

1. **Obligations follow governance** — A Component outside Loft's governance carries no structural obligation.
2. **Room for sharing** — The same field can later say that a Component is shared between documents.
3. **One typed field** — The property is one field with named values, not a flag.
4. **v1 parity** — A converted document yields the structural findings that v1 yields.

##### Options

1. **Boundary kind, inherited** — A `boundary` field of `private` or `external` on any Component, which a Component without it takes from its parent.
2. **External flag** — A boolean `external` field on any Component.
3. **Reserved tag** — A reserved tag name marks an external Component.
4. **v1 scope kinds** — A `scope` field of `product`, `document` or `external` beside the `scoped\_to` edge.

##### Assessment

| Option | Obligations follow governance | Room for sharing | One typed field | v1 parity |
| --- | --- | --- | --- | --- |
| Boundary kind, inherited | supports — the rules skip an external subtree | supports — a `public` value joins the same field | supports — one field with named values | supports — external maps to `external`, and inheritance matches v1's |
| External flag | supports — the rules skip a flagged subtree | conflicts — sharing needs a second field | conflicts — a flag | supports — external maps to the flag |
| Reserved tag | neutral — the rules read a tag name that an author can misspell without an error | neutral — sharing needs another reserved tag | conflicts — a tag carries meaning outside the registry | supports — external maps to the tag |
| v1 scope kinds | supports — the rules read the scope | neutral — sharing becomes a fourth scope value | neutral — one field, but it restates the `scoped\_to` edge | supports — field for field |

##### Decision

Selected: **Boundary kind, inherited**

A Component carries a `boundary` of `private` or `external`, a Component without one takes its parent's, and the obligation rules skip an `external` subtree.

##### Consequences

- **obligation** — The converter gives the boundary `external` to each Component that the export scopes as external. (`RQ-224` · The converter shall give the boundary `external` to each Component that the export scopes as external.)
- **cost** — Every content view gains one slot, so every content hash changes once, before any v2 lock exists.
- **risk** — Adding the value `public` later is a change to the Format.
- **benefit** — A third-party service is modelled without suppressing a finding.

##### Subjects

- **affected** — `CMP-1` · loftspec
- **outcome** — `RQ-221` · The reader shall read a Component's boundary as `private` or `external`.
- **outcome** — `RQ-222` · loftspec shall give a Component without a `boundary` field the boundary of its parent Component.

### Decision History

- **ADR-7 · Lint configuration lives in the repository** — superseded · successor `575101b8-1ce6-4633-96c0-3ccddb685046`
- **ADR-10 · A publish writes the lock only; projections are rendered on demand** — superseded · successor `7475da2b-ed59-4063-ad77-d1728a511847`

## Domain Vocabulary

### conversion

Behaviour that exists only for a document derived from a Loft v1 export.

### git

Behaviour that exists because a git repository holds the history.

### identity

Keys, labels, aliases and the bindings between them.

### portable

Behaviour every implementation must reproduce exactly, so that two implementations compute the same content root for the same document.

## Technical Constraint Categories

_None._

## Realization Links

| Feature | Component |
| --- | --- |
| F-50 · Component boundary | CMP-6 · Validator |
| F-50 · Component boundary | CMP-4 · Reader and writer |
| F-50 · Component boundary | CMP-2 · Document model |
| F-19 · Structural lint rules | CMP-7 · Linter |
| F-6 · Directory layout | CMP-4 · Reader and writer |
| F-13 · Node, registry and link mapping | CMP-5 · Converter |
| F-7 · Order and containment | CMP-4 · Reader and writer |
| F-24 · Markdown projection | CMP-8 · Renderer |
| F-17 · Containment profile | CMP-6 · Validator |
| F-27 · Preconditions | CMP-9 · Publisher |
| F-26 · Projections are never authoritative | CMP-8 · Renderer |
| F-33 · Ordered lists | CMP-10 · Merge engine |
| F-15 · The sidecar | CMP-5 · Converter |
| F-47 · Error positions | CMP-11 · loft-cli |
| F-41 · `loft publish` | CMP-12 · loftspec-store |
| F-41 · `loft publish` | CMP-11 · loft-cli |
| F-4 · Roots | CMP-3 · Hasher |
| F-16 · The round-trip obligation | CMP-5 · Converter |
| F-38 · `loft check` | CMP-11 · loft-cli |
| F-45 · JSON output | CMP-11 · loft-cli |
| F-34 · The typed conflict record | CMP-10 · Merge engine |
| F-5 · Manifest, lock and derived state | CMP-2 · Document model |
| F-23 · Lint configuration in the repository | CMP-7 · Linter |
| F-48 · Decision integrity | CMP-6 · Validator |
| F-44 · Exit codes | CMP-11 · loft-cli |
| F-20 · Language lint rules | CMP-7 · Linter |
| F-14 · Statements and references from prose | CMP-5 · Converter |
| F-43 · `loft merge-driver` | CMP-11 · loft-cli |
| F-3 · Content view and node hash | CMP-3 · Hasher |
| F-46 · Version | CMP-11 · loft-cli |
| F-30 · Idempotence | CMP-9 · Publisher |
| F-39 · `loft fmt` | CMP-12 · loftspec-store |
| F-39 · `loft fmt` | CMP-11 · loft-cli |
| F-37 · `loft import v1` | CMP-12 · loftspec-store |
| F-37 · `loft import v1` | CMP-11 · loft-cli |
| F-32 · Three-way merge over keys | CMP-10 · Merge engine |
| F-21 · Lint rules of v2 | CMP-7 · Linter |
| F-40 · `loft render` | CMP-11 · loft-cli |
| F-1 · Keys | CMP-2 · Document model |
| F-2 · Labels and aliases | CMP-2 · Document model |
| F-22 · Findings and reports | CMP-7 · Linter |
| F-12 · Bodies and sections | CMP-2 · Document model |
| F-28 · Label allocation at publish | CMP-9 · Publisher |
| F-49 · Record fields | CMP-4 · Reader and writer |
| F-18 · Reference resolution | CMP-6 · Validator |
| F-31 · Verification | CMP-9 · Publisher |
| F-8 · Strict reading and no derived data | CMP-4 · Reader and writer |
| F-9 · The YAML subset and canonical layout | CMP-4 · Reader and writer |
| F-11 · Rendering statements | CMP-2 · Document model |
| F-10 · The statement record | CMP-2 · Document model |
| F-29 · The lock | CMP-9 · Publisher |
| F-42 · `loft verify` | CMP-11 · loft-cli |
| F-25 · Canonical JSON export | CMP-8 · Renderer |
| F-35 · The six scenarios | CMP-10 · Merge engine |
| F-36 · `loft init` | CMP-12 · loftspec-store |
| F-36 · `loft init` | CMP-11 · loft-cli |
