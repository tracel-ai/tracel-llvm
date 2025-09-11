//! Async passes.

tracel_mlir_rs_macro::passes!(
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
