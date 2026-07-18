use serde::{Deserialize, Serialize};
use std::fmt;
use ulid::Ulid;

macro_rules! define_bkf_id {
    ($name:ident, $doc:expr) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(pub Ulid);

        impl $name {
            /// Generates a new unique identifier using a chronological, sortable ULID.
            pub fn new() -> Self {
                Self(Ulid::new())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

define_bkf_id!(
    BkfDocumentId,
    "Strongly-typed identifier for a BKF Document."
);
define_bkf_id!(BkfSectionId, "Strongly-typed identifier for a BKF Section.");
define_bkf_id!(BkfBlockId, "Strongly-typed identifier for a BKF Block.");
define_bkf_id!(BkfEntityId, "Strongly-typed identifier for a BKF Entity.");
define_bkf_id!(
    BkfRelationshipId,
    "Strongly-typed identifier for a BKF Relationship."
);
define_bkf_id!(BkfFactId, "Strongly-typed identifier for a BKF Fact.");
define_bkf_id!(
    BkfCitationId,
    "Strongly-typed identifier for a BKF Citation."
);
define_bkf_id!(
    BkfAttachmentId,
    "Strongly-typed identifier for a BKF Attachment."
);
