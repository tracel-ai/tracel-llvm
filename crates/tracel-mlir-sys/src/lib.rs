#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

mod bindings;
#[allow(unused)]
pub use bindings::*;

#[cfg(test)]
mod mlir_tests {
    use super::bindings::*;
    use std::ffi::CString;

    #[test]
    fn test_context() {
        unsafe {
            let ctx = mlirContextCreate();
            assert!(mlirContextEqual(ctx, ctx));
            mlirContextDestroy(ctx);
        }
    }

    #[test]
    fn create_string() {
        unsafe {
            let string = CString::new("Hello, world!").unwrap();

            mlirStringRefCreateFromCString(string.as_ptr());
        }
    }

    #[test]
    fn test_location() {
        unsafe {
            let registry = mlirDialectRegistryCreate();
            let context = mlirContextCreate();

            mlirContextAppendDialectRegistry(context, registry);
            mlirRegisterAllDialects(registry);

            let location = mlirLocationUnknownGet(context);
            let string = CString::new("newmod").unwrap();
            let reference = mlirStringRefCreateFromCString(string.as_ptr());

            mlirOperationStateGet(reference, location);

            mlirContextDestroy(context);
            mlirDialectRegistryDestroy(registry);
        }
    }
}
