//! Traits specifying the interface for database transactions.

/// Low-level interface for read-only database transactions
pub trait TransactionRo: Sized {
    /// Errors that can occur during a transaction.
    type Error;

    /// Finalize the transaction.
    fn finalize(self) -> Result<(), Self::Error>;

    /// Run a read-only transaction.
    ///
    /// High-level convenience method. Prefer this over using a transaction directly.
    ///
    /// ```
    /// # use storage::traits::*;
    /// # fn foo<'a, S: storage::schema::Schema, T: Transactional<'a, S>>(foo: &'a T) {
    /// let result = foo.transaction_ro().run(|tx| {
    ///     // Your transaction operations go here
    ///     Ok(42) // this will be the result
    /// });
    /// # }
    /// ```
    ///
    /// Implementations are allowed to override this method provided semantics are preserved.
    fn run<R>(
        self,
        tx_body: impl FnOnce(&Self) -> Result<R, Self::Error>,
    ) -> Result<R, Self::Error> {
        let result = tx_body(&self);
        self.finalize()?;
        result
    }
}

/// Low-level interface for read-write database transactions
pub trait TransactionRw: Sized {
    /// Errors that can occur during a transaction.
    type Error;

    /// Commit a transaction
    fn commit(self) -> Result<(), Self::Error>;

    /// Abort a transaction.
    fn abort(self) -> Result<(), Self::Error>;

    /// Run a read-write transaction.
    ///
    /// High-level convenience method. Prefer this over using the transaction directly.
    ///
    /// ```
    /// # use storage::traits::*;
    /// # fn foo<'a, S: storage::schema::Schema, T: Transactional<'a, S>>(foo: &'a mut T) {
    /// let result = foo.transaction_rw().run(|tx| {
    ///     // Your transaction operations go here
    ///     storage::commit(42) // this will be the result
    /// });
    /// # }
    /// ```
    ///
    /// Implementations are allowed to override this method provided semantics are preserved.
    fn run<R>(
        mut self,
        tx_body: impl FnOnce(&mut Self) -> Result<Response<R>, Self::Error>,
    ) -> Result<R, Self::Error> {
        let result = tx_body(&mut self);
        match result {
            Ok(Response::Commit(_)) => self.commit()?,
            Ok(Response::Abort(_)) => self.abort()?,
            Err(_) => (),
        };
        result.map(Response::value)
    }
}

/// Transaction response
#[must_use = "Response must be returned from the transaction"]
pub enum Response<T> {
    /// Commit the transaction to the storage
    Commit(T),
    /// Abort the transaction
    Abort(T),
}

impl<T> Response<T> {
    // Extract the return value from the response.
    fn value(self) -> T {
        match self {
            Self::Commit(v) => v,
            Self::Abort(v) => v,
        }
    }
}

/// Commit a transaction, returning given value
#[must_use = "commit must be returned from the transaction"]
pub fn commit<T, E>(ret: T) -> Result<Response<T>, E> {
    Ok(Response::Commit(ret))
}

/// Abort a transaction, returning given value
#[must_use = "abort must be returned from the transaction"]
pub fn abort<T, E>(ret: T) -> Result<Response<T>, E> {
    Ok(Response::Abort(ret))
}
