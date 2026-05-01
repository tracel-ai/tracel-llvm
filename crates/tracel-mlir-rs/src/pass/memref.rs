//! MemRef passes.

tracel_mlir_rs_macros::passes!(
    "MemRef",
    [
        // spell-checker: disable-next-line
        mlirCreateMemRefExpandOpsPass,
        mlirCreateMemRefExpandReallocPass,
        mlirCreateMemRefExpandStridedMetadataPass,
        mlirCreateMemRefFlattenMemrefsPass,
        mlirCreateMemRefFoldMemRefAliasOpsPass,
        mlirCreateMemRefNormalizeMemRefsPass,
        mlirCreateMemRefReifyResultShapesPass,
        mlirCreateMemRefResolveRankedShapeTypeResultDimsPass,
        mlirCreateMemRefResolveShapedTypeResultDimsPass,
    ]
);
