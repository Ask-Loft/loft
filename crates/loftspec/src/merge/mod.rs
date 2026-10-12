//! The merge engine (CMP-10): a three-way merge of one file of a `spec/` tree, over keys and
//! ordered lists, writing a typed conflict record where the two sides disagree (D-11 F-32..F-34).

pub mod order;
