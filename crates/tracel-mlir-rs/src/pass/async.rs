//! Async passes.

tracel_mlir_rs_macros::passes!(
    "Async",
    [
        mlirCreateAsyncAsyncFuncToAsyncRuntimePass,
        mlirCreateAsyncAsyncParallelForPass,
        mlirCreateAsyncAsyncRuntimePolicyBasedRefCountingPass,
        mlirCreateAsyncAsyncRuntimeRefCountingPass,
        mlirCreateAsyncAsyncRuntimeRefCountingOptPass,
        mlirCreateAsyncAsyncToAsyncRuntimePass,
    ]
);
