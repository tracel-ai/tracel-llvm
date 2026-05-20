use crate::{
    dialect::llvm::attributes::{atomic_ordering, AtomicOrdering},
    ir::{
        attribute::{ArrayAttribute, IntegerAttribute, StringAttribute},
        Attribute, Identifier,
    },
    Context,
};

const ATTRIBUTE_COUNT: usize = 10;

/// Compare-exchange options.
#[derive(Debug, Default, Clone, Copy)]
pub struct CmpXchgOptions<'c> {
    align: Option<IntegerAttribute<'c>>,
    volatile: bool,
    weak: bool,
    syncscope: Option<StringAttribute<'c>>,
    access_groups: Option<ArrayAttribute<'c>>,
    alias_scopes: Option<ArrayAttribute<'c>>,
    noalias_scopes: Option<ArrayAttribute<'c>>,
    tbaa: Option<ArrayAttribute<'c>>,
}

impl<'c> CmpXchgOptions<'c> {
    /// Creates compare-exchange options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets an alignment.
    pub fn align(mut self, align: Option<IntegerAttribute<'c>>) -> Self {
        self.align = align;
        self
    }

    /// Sets a volatile flag.
    pub fn volatile(mut self, volatile: bool) -> Self {
        self.volatile = volatile;
        self
    }

    /// Sets a weak flag.
    pub fn weak(mut self, weak: bool) -> Self {
        self.weak = weak;
        self
    }

    /// Sets a synchronization scope.
    pub fn syncscope(mut self, syncscope: Option<StringAttribute<'c>>) -> Self {
        self.syncscope = syncscope;
        self
    }

    /// Sets access groups.
    pub fn access_groups(mut self, access_groups: Option<ArrayAttribute<'c>>) -> Self {
        self.access_groups = access_groups;
        self
    }

    /// Sets alias scopes.
    pub fn alias_scopes(mut self, alias_scopes: Option<ArrayAttribute<'c>>) -> Self {
        self.alias_scopes = alias_scopes;
        self
    }

    /// Sets noalias scopes.
    pub fn noalias_scopes(mut self, noalias_scopes: Option<ArrayAttribute<'c>>) -> Self {
        self.noalias_scopes = noalias_scopes;
        self
    }

    /// Sets TBAA metadata.
    pub const fn tbaa(mut self, tbaa: ArrayAttribute<'c>) -> Self {
        self.tbaa = Some(tbaa);
        self
    }

    pub(super) fn into_attributes(
        self,
        context: &'c Context,
        success_ordering: AtomicOrdering,
        failure_ordering: AtomicOrdering,
    ) -> Vec<(Identifier<'c>, Attribute<'c>)> {
        let mut attributes = Vec::with_capacity(ATTRIBUTE_COUNT);

        attributes.push((
            Identifier::new(context, "success_ordering"),
            atomic_ordering(context, success_ordering),
        ));
        attributes.push((
            Identifier::new(context, "failure_ordering"),
            atomic_ordering(context, failure_ordering),
        ));

        if let Some(syncscope) = self.syncscope {
            attributes.push((Identifier::new(context, "syncscope"), syncscope.into()));
        }

        if let Some(align) = self.align {
            attributes.push((Identifier::new(context, "alignment"), align.into()));
        }

        if self.weak {
            attributes.push((Identifier::new(context, "weak"), Attribute::unit(context)));
        }

        if self.volatile {
            attributes.push((
                Identifier::new(context, "volatile_"),
                Attribute::unit(context),
            ));
        }

        if let Some(access_groups) = self.access_groups {
            attributes.push((
                Identifier::new(context, "access_groups"),
                access_groups.into(),
            ));
        }

        if let Some(alias_scopes) = self.alias_scopes {
            attributes.push((
                Identifier::new(context, "alias_scopes"),
                alias_scopes.into(),
            ));
        }

        if let Some(noalias_scopes) = self.noalias_scopes {
            attributes.push((
                Identifier::new(context, "noalias_scopes"),
                noalias_scopes.into(),
            ));
        }

        if let Some(tbaa) = self.tbaa {
            attributes.push((Identifier::new(context, "tbaa"), tbaa.into()));
        }

        attributes
    }
}
