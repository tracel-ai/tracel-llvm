//! Arith passes.

tracel_mlir_rs_macros::passes!(
    "Arith",
    [
        // spell-checker: disable-next-line
        mlirCreateArithArithEmulateUnsupportedFloats,
        mlirCreateArithArithEmulateWideInt,
        mlirCreateArithArithExpandOpsPass,
        mlirCreateArithArithIntRangeNarrowing,
        mlirCreateArithArithIntRangeOpts,
        mlirCreateArithArithUnsignedWhenEquivalentPass,
    ]
);
