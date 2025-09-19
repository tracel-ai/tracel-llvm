//! Sparse tensor passes.

tracel_mlir_rs_macros::passes!(
    "SparseTensor",
    [
        mlirCreateSparseTensorLowerForeachToSCF,
        mlirCreateSparseTensorLowerSparseOpsToForeach,
        mlirCreateSparseTensorPreSparsificationRewrite,
        mlirCreateSparseTensorSparseBufferRewrite,
        mlirCreateSparseTensorSparseGPUCodegen,
        mlirCreateSparseTensorSparseReinterpretMap,
        mlirCreateSparseTensorSparseTensorCodegen,
        mlirCreateSparseTensorSparseTensorConversionPass,
        mlirCreateSparseTensorSparseVectorization,
        mlirCreateSparseTensorSparsificationAndBufferization,
        mlirCreateSparseTensorSparsificationPass,
        mlirCreateSparseTensorStageSparseOperations,
        mlirCreateSparseTensorStorageSpecifierToLLVM,
    ]
);
