//! Experimental dialect operations and their builders generated automatically
//! from TableGen files.

#[doc(hidden)]
pub mod __private {
    pub struct Set;
    pub struct Unset;
}

tracel_mlir_rs_macros::dialect! {
    name: "affine",
    files: ["IR/AffineOps.td", "TransformOps/AffineTransformOps.td", "IR/AffineMemoryOpInterfaces.td"],
    include_directories: ["mlir/Dialect/Affine"],
}

/* TODO: Fix "error: invalid conversion from Invalid to alloc::string::String" probably tblgen issue?
tracel_mlir_rs_macros::dialect! {
    name: "amdgpu",
    files: ["IR/AMDGPU.td", "Transforms/Passes.td"],
    include_directories: ["mlir/Dialect/AMDGPU"],
}
*/

tracel_mlir_rs_macros::dialect! {
    name: "arith",
    files: ["mlir/Dialect/Arith/IR/ArithOps.td"],
}

tracel_mlir_rs_macros::dialect! {
    name: "arm_neon",
    files: ["mlir/Dialect/ArmNeon/ArmNeon.td"],
}

tracel_mlir_rs_macros::dialect! {
    name: "arm_sve",
    files: ["mlir/Dialect/ArmSVE/IR/ArmSVE.td"],
}

tracel_mlir_rs_macros::dialect! {
    name: "arm_sme",
    files: ["ArmSME.td", "ArmSMEOps.td", "ArmSMEIntrinsicOps.td"],
    include_directories: ["mlir/Dialect/ArmSME/IR"],
}

tracel_mlir_rs_macros::dialect! {
    name: "async",
    files: ["AsyncDialect.td", "AsyncOps.td", "AsyncTypes.td"],
    include_directories: ["mlir/Dialect/Async/IR"],
}

tracel_mlir_rs_macros::dialect! {
    name: "amx",
    files: ["mlir/Dialect/AMX/AMX.td"],
}

tracel_mlir_rs_macros::dialect! {
    name: "builtin",
    files: ["mlir/IR/BuiltinOps.td"],
}

tracel_mlir_rs_macros::dialect! {
    name: "bufferization",
    files: [
        "IR/BufferizationOps.td",
        "IR/AllocationOpInterface.td",
        "IR/BufferizationEnums.td",
        "IR/BufferizableOpInterface.td",
        "TransformOps/BufferizationTransformOps.td",
        "Transforms/Passes.td",
    ],
    include_directories: ["mlir/Dialect/Bufferization"],
}

tracel_mlir_rs_macros::dialect! {
    name: "complex",
    files: ["ComplexBase.td", "ComplexOps.td"],
    include_directories: ["mlir/Dialect/Complex/IR"],
}

tracel_mlir_rs_macros::dialect! {
    name: "cf",
    files: ["mlir/Dialect/ControlFlow/IR/ControlFlowOps.td"],
}

tracel_mlir_rs_macros::dialect! {
    name: "dlti",
    files: ["DLTI.td", "DLTIAttrs.td", "DLTIBase.td"],
    include_directories: ["mlir/Dialect/DLTI"]
}

tracel_mlir_rs_macros::dialect! {
    name: "func",
    files: ["IR/FuncOps.td", "TransformOps/FuncTransformOps.td", "Transforms/Passes.td"],
    include_directories: ["mlir/Dialect/Func"],
}

tracel_mlir_rs_macros::dialect! {
    name: "index",
    files: ["mlir/Dialect/Index/IR/IndexOps.td"],
}

tracel_mlir_rs_macros::dialect! {
    name: "irdl",
    files: ["IRDLOps.td", "IRDL.td"],
    include_directories: ["mlir/Dialect/IRDL/IR"],
}

tracel_mlir_rs_macros::dialect! {
    name: "llvm",
    // spell-checker: disable-next-line
    files: [
        "LLVMOps.td",
        "LLVMIntrinsicOps.td",
        "LLVMDialect.td",
        "LLVMInterfaces.td",
        "LLVMTypes.td",
        "LLVMEnums.td",
        "LLVMOpBase.td",
        "LLVMAttrDefs.td",
        "BasicPtxBuilderInterface.td",
    ],
    include_directories: ["mlir/Dialect/LLVMIR"],
}

tracel_mlir_rs_macros::dialect! {
    name: "memref",
    files: ["mlir/Dialect/MemRef/IR/MemRefOps.td"],
}

tracel_mlir_rs_macros::dialect! {
    name: "scf",
    files: ["mlir/Dialect/SCF/IR/SCFOps.td"],
}

tracel_mlir_rs_macros::dialect! {
    name: "pdl",
    files: ["mlir/Dialect/PDL/IR/PDLOps.td"],
}

tracel_mlir_rs_macros::dialect! {
    name: "pdl_interp",
    files: ["mlir/Dialect/PDLInterp/IR/PDLInterpOps.td"],
}

tracel_mlir_rs_macros::dialect! {
    name: "math",
    files: ["mlir/Dialect/Math/IR/MathOps.td"],
}

tracel_mlir_rs_macros::dialect! {
    name: "gpu",
    files: ["mlir/Dialect/GPU/IR/GPUOps.td"],
}

tracel_mlir_rs_macros::dialect! {
    name: "linalg",
    files: ["mlir/Dialect/Linalg/IR/LinalgOps.td"],
}

tracel_mlir_rs_macros::dialect! {
    name: "quant",
    files: ["IR/QuantOps.td", "Transforms/Passes.td"],
    include_directories: ["mlir/Dialect/Quant"],
}

tracel_mlir_rs_macros::dialect! {
    name: "shape",
    files: ["mlir/Dialect/Shape/IR/ShapeOps.td"],
}

tracel_mlir_rs_macros::dialect! {
    name: "sparse_tensor",
    files: ["mlir/Dialect/SparseTensor/IR/SparseTensorOps.td"],
}

tracel_mlir_rs_macros::dialect! {
    name: "tensor",
    files: ["mlir/Dialect/Tensor/IR/TensorOps.td"],
}

/* TODO: Fix "error: invalid conversion from Invalid to alloc::string::String" probably tblgen issue?
tracel_mlir_rs_macros::dialect! {
    name: "tosa",
    files: ["mlir/Dialect/Tosa/IR/TosaOps.td"],
}
*/

tracel_mlir_rs_macros::dialect! {
    name: "transform",
    files: ["mlir/Dialect/Transform/IR/TransformOps.td"],
}

tracel_mlir_rs_macros::dialect! {
    name: "vector",
    files: ["mlir/Dialect/Vector/IR/VectorOps.td"],
}

tracel_mlir_rs_macros::dialect! {
    name: "x86vector",
    files: ["mlir/Dialect/X86Vector/X86Vector.td"],
}

#[cfg(test)]
mod tests {
    use crate::{
        dialect::{arith, func},
        ir::{
            attribute::{StringAttribute, TypeAttribute},
            operation::OperationLike,
            r#type::FunctionType,
            Block, BlockLike, Location, Module, Region, RegionLike, Type,
        },
        pass::{self, PassManager},
        test::create_test_context,
        Context,
    };

    fn convert_module<'c>(context: &'c Context, module: &mut Module<'c>) {
        let pass_manager = PassManager::new(context);

        pass_manager.add_pass(pass::conversion::create_func_to_llvm());
        pass_manager
            .nested_under("func.func")
            .add_pass(pass::conversion::create_arith_to_llvm());
        pass_manager
            .nested_under("func.func")
            .add_pass(pass::conversion::create_index_to_llvm());
        pass_manager.add_pass(pass::conversion::create_scf_to_control_flow());
        pass_manager.add_pass(pass::conversion::create_control_flow_to_llvm());
        pass_manager.add_pass(pass::conversion::create_finalize_mem_ref_to_llvm());

        assert_eq!(pass_manager.run(module), Ok(()));
        assert!(module.as_operation().verify());
    }

    fn test_operation<'c>(
        name: &str,
        context: &'c Context,
        argument_types: &[Type<'c>],
        callback: impl FnOnce(&Block<'c>),
    ) {
        let location = Location::unknown(context);
        let mut module = Module::new(location);

        module.body().append_operation(func::func(
            context,
            StringAttribute::new(context, "foo"),
            TypeAttribute::new(FunctionType::new(context, argument_types, &[]).into()),
            {
                let block = Block::new(
                    &argument_types
                        .iter()
                        .copied()
                        .map(|r#type| (r#type, location))
                        .collect::<Vec<_>>(),
                );

                callback(&block);

                let region = Region::new();
                region.append_block(block);
                region
            },
            &[],
            location,
        ));

        convert_module(context, &mut module);

        assert!(module.as_operation().verify());
        insta::assert_snapshot!(name, module.as_operation());
    }

    #[test]
    fn compile_arith_addf() {
        let context = create_test_context();
        let location = Location::unknown(&context);
        let r#type = Type::float32(&context);

        test_operation("addf", &context, &[r#type, r#type], |block| {
            block.append_operation(arith::addf(
                block.argument(0).unwrap().into(),
                block.argument(1).unwrap().into(),
                location,
            ));

            block.append_operation(func::r#return(&[], location));
        });
    }

    #[test]
    fn compile_arith_addf_builder_with_reverse_order() {
        todo!("Fix this function");
        // let context = create_test_context();
        // let location = Location::unknown(&context);
        // let r#type = Type::float32(&context);

        // test_operation("addf_builder", &context, &[r#type, r#type], |block| {
        //     block.append_operation(
        //         arith::AddFOperationBuilder::new(&context, location)
        //             .lhs(block.argument(0).unwrap().into())
        //             .rhs(block.argument(1).unwrap().into())
        //             .build()
        //             .into(),
        //     );

        //     block.append_operation(func::r#return(&context, &[], location).into());
        // });
    }

    #[test]
    fn compile_llvm_alloca() {
        todo!("Fix this test");
        // let context = create_test_context();
        // let location = Location::unknown(&context);
        // let integer_type = IntegerType::new(&context, 64).into();

        // test_operation("alloc", &context, &[integer_type], |block| {
        //     let alloca_size = block.argument(0).unwrap().into();

        //     block.append_operation(
        //         llvm::AllocaOperation::builder(&context, location)
        //             .array_size(alloca_size)
        //             .elem_type(TypeAttribute::new(integer_type))
        //             .res(dialect::llvm::r#type::pointer(&context, 0))
        //             .build()
        //             .into(),
        //     );

        //     block.append_operation(func::r#return(&context, &[], location).into());
        // });
    }

    #[test]
    fn compile_llvm_alloca_builder() {
        todo!("Fix this test");
        // let context = create_test_context();
        // let location = Location::unknown(&context);
        // let integer_type = IntegerType::new(&context, 64).into();
        // let ptr_type = dialect::llvm::r#type::pointer(&context, 0);

        // test_operation("alloc_builder", &context, &[integer_type], |block| {
        //     let alloca_size = block.argument(0).unwrap().into();

        //     block.append_operation(
        //         llvm::AllocaOperationBuilder::new(&context, location)
        //             .alignment(IntegerAttribute::new(integer_type, 8))
        //             .elem_type(TypeAttribute::new(integer_type))
        //             .array_size(alloca_size)
        //             .res(ptr_type)
        //             .build()
        //             .into(),
        //     );

        //     block.append_operation(func::r#return(&context, &[], location).into());
        // });
    }
}
