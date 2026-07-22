use brain_core::repositories::RepositorySet;

/// An immutable read-only snapshot of the knowledge graph for reflection passes.
///
/// Ensures all analysis passes in a single reflection cycle inspect the exact same revision
/// of the graph without seeing intermediate mutations.
pub struct ReflectionSnapshot<'a> {
    repositories: &'a dyn RepositorySet,
}

impl<'a> ReflectionSnapshot<'a> {
    /// Creates a new `ReflectionSnapshot` wrapping a read-only repository set reference.
    pub fn new(repositories: &'a dyn RepositorySet) -> Self {
        Self { repositories }
    }

    /// Accesses the read-only repository set.
    pub fn repositories(&self) -> &'a dyn RepositorySet {
        self.repositories
    }
}
