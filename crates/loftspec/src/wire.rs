//! Wire names: the one place each closed set of values is spelled as the text form and the
//! hash views write it. `wire_names!` gives a fieldless enum its members in order, their names,
//! and the conversions both ways, so a reader, a writer and a schema table cannot disagree.

/// Implements `ALL`, `NAMES`, `as_str` and `parse` for a fieldless enum declared elsewhere.
macro_rules! wire_names {
    ($name:ty { $($variant:ident => $text:literal),+ $(,)? }) => {
        impl $name {
            /// Every value, in declaration order.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// Every value's wire name, in declaration order.
            pub const NAMES: &'static [&'static str] = &[$($text),+];

            /// The value's wire name.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $text),+
                }
            }

            /// The value a wire name names; none for a name the set lacks.
            #[must_use]
            pub fn parse(text: &str) -> Option<Self> {
                match text {
                    $($text => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }
    };
}

pub(crate) use wire_names;
