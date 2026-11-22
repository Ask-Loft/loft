//! Which links the page keeps. comrak keeps a link to any address, as cmark-gfm does, and its
//! autolink extension links a `www.` address and a bare email address as well, as GitHub does.
//! The page keeps a link only to an address of the web, of mail or within the page: no script
//! runs from a block, nothing is loaded from elsewhere, and no link leads to a file the page
//! server does not serve. It draws an image as a link to it, and a `www.` address and a bare
//! email address as text, as the specification chooses.

use comrak::nodes::{Node, NodeValue};

/// Turns each image under `root` that the page keeps into a link to it, and each link or image it
/// does not keep into its text.
pub fn keep_safe(root: Node<'_>) {
    // Collected first: undoing a link moves its text, which would cut a walk of the tree short.
    let links: Vec<(Node<'_>, String)> = root
        .descendants()
        .filter_map(|node| match &node.data().value {
            NodeValue::Link(link) | NodeValue::Image(link) => Some((node, link.url.clone())),
            _ => None,
        })
        .collect();
    for (node, url) in links {
        if is_safe(&url) && !is_bare_www_or_email(node, &url) && !is_within_a_link(node) {
            let mut data = node.data_mut();
            if let NodeValue::Image(link) = &data.value {
                data.value = NodeValue::Link(link.clone());
            }
        } else {
            undo(node);
        }
    }
    move_footnotes_out_of_links(root);
}

/// Puts each footnote's reference that lies within a link right after that link: comrak writes
/// the reference as a link of its own, and one link within another a browser splits in two.
fn move_footnotes_out_of_links(root: Node<'_>) {
    let footnotes: Vec<Node<'_>> = root
        .descendants()
        .filter(|node| matches!(node.data().value, NodeValue::FootnoteReference(_)))
        .collect();
    // From the last, so that two references within one link keep their order after it.
    for footnote in footnotes.into_iter().rev() {
        let link = footnote
            .ancestors()
            .skip(1)
            .find(|above| matches!(above.data().value, NodeValue::Link(_)));
        if let Some(link) = link {
            link.insert_after(footnote);
        }
    }
}

/// Whether `node`, a link or an image, lies within a link: as a link of its own it would make one
/// link within another, which a browser splits in two. The walk reaches a node after those above
/// it, so an image above it is a link by then, and one undone is no longer above it.
fn is_within_a_link(node: Node<'_>) -> bool {
    node.ancestors()
        .skip(1)
        .any(|above| matches!(above.data().value, NodeValue::Link(_)))
}

/// Whether `node`, a link to `url`, is one the autolink extension made of a `www.` address or a
/// bare email address. Such a link stands exactly where its one text does, as no link written
/// with `[…](…)` or `<…>` does, and its address is that text with `http://` or `mailto:` before
/// it, where the extension's link of an `http`, `https` or `mailto` address is its text alone.
fn is_bare_www_or_email(node: Node<'_>, url: &str) -> bool {
    let Some(text) = node
        .first_child()
        .filter(|text| text.next_sibling().is_none())
    else {
        return false;
    };
    let text = text.data();
    let NodeValue::Text(words) = &text.value else {
        return false;
    };
    text.sourcepos == node.data().sourcepos && words.as_ref() != url
}

/// Whether a link may keep `address`: one of the web or of mail, or a place within the page.
fn is_safe(address: &str) -> bool {
    address.starts_with('#')
        || ["https://", "http://", "mailto:"].iter().any(|scheme| {
            address
                .get(..scheme.len())
                .is_some_and(|head| head.eq_ignore_ascii_case(scheme))
        })
}

/// Puts `node`'s children in its place, so that its text stays and the link goes.
fn undo(node: Node<'_>) {
    while let Some(child) = node.first_child() {
        node.insert_before(child);
    }
    node.detach();
}

#[cfg(test)]
mod tests;
