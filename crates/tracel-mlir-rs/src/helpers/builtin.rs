use crate::{
    ir::{block::BlockLike, Block, Operation, Value},
    Error,
};

/// A block extension for a `builtin` dialect and constructs.
pub trait BuiltinBlockExt<'c> {
    /// Returns a block argument as a value.
    fn arg(&self, index: usize) -> Result<Value<'c, '_>, Error>;

    /// Appends an operation and returns its first value.
    fn append_op_result(&self, operation: impl Into<Operation<'c>>)
        -> Result<Value<'c, 'c>, Error>;
}

impl<'c> BuiltinBlockExt<'c> for Block<'c> {
    #[inline]
    fn arg(&self, index: usize) -> Result<Value<'c, '_>, Error> {
        Ok(self.argument(index)?.into())
    }

    #[inline]
    fn append_op_result(
        &self,
        operation: impl Into<Operation<'c>>,
    ) -> Result<Value<'c, 'c>, Error> {
        Ok(self.append_operation(operation).result(0)?.into())
    }
}
