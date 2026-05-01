//! GPU passes.

tracel_mlir_rs_macros::passes!(
    "GPU",
    [
        // spell-checker: disable-next-line
        mlirCreateGPUGpuAsyncRegionPass,
        mlirCreateGPUGpuDecomposeMemrefsPass,
        mlirCreateGPUGpuEliminateBarriers,
        mlirCreateGPUGpuKernelOutliningPass,
        mlirCreateGPUGpuLaunchSinkIndexComputationsPass,
        mlirCreateGPUGpuMapParallelLoopsPass,
        mlirCreateGPUGpuModuleToBinaryPass,
        mlirCreateGPUGpuNVVMAttachTarget,
        mlirCreateGPUGpuROCDLAttachTarget,
        mlirCreateGPUGpuSPIRVAttachTarget,
    ]
);
