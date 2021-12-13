#![allow(type_alias_bounds)]

use crate::schema::{self, Column, Schema};

pub type Data = Vec<u8>;

/*
pub trait MapForMultiplicity<'tx, MapKind> {
    type MapType;
}

impl<'tx, B: SingleMap<'tx>> MapForMultiplicity<'tx, map_kind::Single> for B {
    type MapType = B::Impl;
}

impl<'tx, B: MultiMap<'tx>> MapForMultiplicity<'tx, map_kind::Multi> for B {
    type MapType = B::Impl;
}

pub type MapFor<'tx, Col: Column, B: MapForMultiplicity<'tx, Col::MapKind>> = B::MapType;
*/

/// Transaction type associated with given backend
pub trait Transaction<'st, Sch: Schema> {
    type Impl;
    // start
    // commit
    // abort
    // get_multi
    /*
    fn get<'tx, Col: Column>(this: &'tx TxImpl<'st, Sch, Self>) -> MapFor<'tx, Col, Self>
        where Self: MapForMultiplicity<'tx, Col::MapKind>;
    */
    fn get_single<'tx, I>(this: &'tx mut TxImpl<'st, Sch, Self>) -> <Self as SingleMap<'tx>>::Impl
    where
        Self: SingleMap<'tx> + GetMap<'st, 'tx, Sch, I>,
        'st: 'tx;
}

pub trait GetMap<'st: 'tx, 'tx, Sch: Schema, I>: SingleMap<'tx> {
    type Impl;

    fn get_single(
        txref: <Self as GetMap<'st, 'tx, Sch, I>>::Impl,
    ) -> <Self as SingleMap<'tx>>::Impl;
}

pub type TxImpl<'st, Sch: Schema, T: Transaction<'st, Sch>> = T::Impl;

/// Map taking a key to a value
pub trait SingleMap<'tx> {
    /// The implementation type
    type Impl;

    /// Check if the key exists in the map
    fn exists(e: &Self::Impl, key: &[u8]) -> crate::Result<bool>;

    /// Get the value.
    fn get<'a, 'b>(e: &'a Self::Impl, key: &'b [u8]) -> crate::Result<Option<&'a [u8]>>;

    /// Put value into the entry
    fn put(e: &mut Self::Impl, key: &[u8], val: &[u8]) -> crate::Result<()>;

    /// Remove the value
    fn remove(e: &mut Self::Impl, key: &[u8]) -> crate::Result<()>;
}

//pub type SingleMapImpl<'m, E: SingleMap<'m>> = E::Impl;

/// Map taking a key to multiple values
pub trait MultiMap<'tx> {
    /// The implementation type
    type Impl;

    /*
    /// Check if the key exists in the map
    fn exists(e: &Self::Impl, key: &[u8]) -> crate::Result<bool>;

    /// Get the value.
    fn get<'a, 'b>(e: &'a Self::Impl, key: &'b [u8]) -> crate::Result<Option<&'a [u8]>>;

    /// Put value into the entry
    fn put(e: &mut Self::Impl, key: Data, val: Data) -> crate::Result<()>;

    /// Remove the value
    fn remove(e: &mut Self::Impl, key: &[u8]) -> crate::Result<()>;
    */
}

//pub type MultiMapImpl<'m, E: MultiMap<'m>> = E::Impl;

/*

/// Schema-independent map types for given
pub trait StoreMaps<'tx> {
    type SingleEntry: for<'a> SingleEntry<'a>;
    type SingleMap: Map<'tx, Entry = Self::SingleEntry>;
//    type MultiEntry: MultiEntry
}

/// Bag of values associated with one key
pub trait MultiEntry {
    // TODO
}
*/

#[cfg(test)]
mod test {}
