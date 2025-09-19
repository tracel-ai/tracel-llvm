//! Async passes.

tracel_mlir_rs_macros::passes!(
    "Async",
    [
        mlirCreateAsyncAsyncFuncToAsyncRuntime,
        mlirCreateAsyncAsyncParallelFor,
        mlirCreateAsyncAsyncRuntimePolicyBasedRefCounting,
        mlirCreateAsyncAsyncRuntimeRefCounting,
        mlirCreateAsyncAsyncRuntimeRefCountingOpt,
        mlirCreateAsyncAsyncToAsyncRuntime,
    ]
);
