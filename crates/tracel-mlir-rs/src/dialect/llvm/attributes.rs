use crate::{
    ir::{attribute::IntegerAttribute, r#type::IntegerType, Attribute},
    Context,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Linkage {
    Private,
    Internal,
    AvailableExternally,
    LinkOnce,
    Weak,
    Common,
    Appending,
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtomicOrdering {
    Unordered,
    Monotonic,
    Acquire,
    Release,
    AcquireRelease,
    SequentiallyConsistent,
}

/// Creates an LLVM linkage attribute.
pub fn linkage(context: &Context, linkage: Linkage) -> Attribute<'_> {
    let linkage = match linkage {
        Linkage::Private => "private",
        Linkage::Internal => "internal",
        Linkage::AvailableExternally => "available_externally",
        Linkage::LinkOnce => "link_once",
        Linkage::Weak => "weak",
        Linkage::Common => "common",
        Linkage::Appending => "appending",
        Linkage::External => "external",
    };
    Attribute::parse(context, &format!("#llvm.linkage<{linkage}>")).unwrap()
}

/// Creates an LLVM atomic ordering attribute.
pub fn atomic_ordering(context: &Context, ordering: AtomicOrdering) -> Attribute<'_> {
    let value = match ordering {
        AtomicOrdering::Unordered => 1,
        AtomicOrdering::Monotonic => 2,
        AtomicOrdering::Acquire => 4,
        AtomicOrdering::Release => 5,
        AtomicOrdering::AcquireRelease => 6,
        AtomicOrdering::SequentiallyConsistent => 7,
    };

    IntegerAttribute::new(IntegerType::new(context, 64).into(), value).into()
}
