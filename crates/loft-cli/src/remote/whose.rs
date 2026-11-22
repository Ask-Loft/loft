//! Whose remote a sign-in reaches: the document's own, or another document's that `loft
//! interface` reaches. It words what mends an address that may not be a document's remote, since
//! the document's own address lies in the manifest's `remote` and another's in its entry of
//! `documents`.

use super::address_form_for;

/// Whose remote a sign-in reaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Whose<'a> {
    /// The document's own remote; the document's key.
    Own { key: &'a str },
    /// Another document's remote, listed under `handle`, with that document's key where
    /// `documents` lists it already.
    Other {
        handle: &'a str,
        key: Option<&'a str>,
    },
}

impl Whose<'_> {
    /// The address a Loft server gives the document, and what mends an address that may not be a
    /// document's remote.
    #[must_use]
    pub fn mend_address(self) -> String {
        match self {
            Self::Own { key } => format!(
                "A Loft server gives this document the address {}. `loft remote set` with the \
                 document's address replaces the `remote` that spec/loftspec.yaml names",
                address_form_for(key)
            ),
            Self::Other {
                handle,
                key: Some(key),
            } => format!(
                "A Loft server gives that document the address {}. Correct the address in the \
                 entry of `documents` for {handle} in spec/loftspec.yaml",
                address_form_for(key)
            ),
            Self::Other { key: None, .. } => format!(
                "A Loft server gives a document the address {}, where <documentKey> is that \
                 document's key. `--remote` takes the other document's address",
                super::document_address_form!()
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Whose;

    #[test]
    fn each_remote_s_address_is_mended_where_it_is_written() {
        let own = Whose::Own { key: "k" }.mend_address();
        assert!(own.contains("documents/k/remote") && own.contains("`loft remote set`"));
        let listed = Whose::Other {
            handle: "shelves",
            key: Some("k"),
        };
        let listed = listed.mend_address();
        assert!(listed.contains("documents/k/remote"), "{listed}");
        assert!(listed.contains("`documents` for shelves") && !listed.contains("loft remote"));
        let unlisted = Whose::Other {
            handle: "shelves",
            key: None,
        };
        let unlisted = unlisted.mend_address();
        assert!(
            unlisted.contains("documents/<documentKey>/remote"),
            "{unlisted}"
        );
        assert!(unlisted.contains("`--remote`") && !unlisted.contains("loft remote"));
    }
}
