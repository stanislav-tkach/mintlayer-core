//! The internal implementation of the in-memory storage backend.

#![allow(type_alias_bounds)]

use std::collections::BTreeMap;
use storage::schema::{self, Column, Schema};
use storage::traits::Data;

/// Get map type associated with given map kind
trait MapTy {
    type StoreMap;
    type DeltaMap;
}

type StoreMapSingle = BTreeMap<Data, Data>;
type DeltaMapSingle = BTreeMap<Data, Option<Data>>;
type StoreMapMulti = BTreeMap<Data, Vec<Data>>;
type DeltaMapMulti = BTreeMap<Data, ()>; // TODO

impl MapTy for schema::Single {
    type StoreMap = StoreMapSingle;
    type DeltaMap = DeltaMapSingle;
}

impl MapTy for schema::Multi {
    type StoreMap = StoreMapMulti;
    type DeltaMap = DeltaMapMulti;
}

// Lookup column type by Column
trait ColumnInfo: Column {
    type StoreMap;
    type DeltaMap;
}

// Get map type associated with given column
impl<Col: Column> ColumnInfo for Col
where
    Col::Kind: MapTy,
{
    type StoreMap = <Col::Kind as MapTy>::StoreMap;
    type DeltaMap = <Col::Kind as MapTy>::DeltaMap;
}

/// Store with an extra column
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct StoreExt<Col: ColumnInfo, Rest>(Col::StoreMap, Rest);

/// Delta tracker with an extra column
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct DeltaExt<Col: ColumnInfo, Rest>(Col::DeltaMap, Rest);

trait SchemaInfo: Schema {
    type Store;
    type Delta;
}

impl SchemaInfo for () {
    type Store = ();
    type Delta = ();
}

impl<Col: ColumnInfo, Rest: SchemaInfo> SchemaInfo for (Col, Rest) {
    type Store = StoreExt<Col, Rest::Store>;
    type Delta = DeltaExt<Col, Rest::Delta>;
}

#[derive(Debug)]
struct Transaction<'st, Sch: SchemaInfo> {
    store: &'st mut Sch::Store,
    delta: Sch::Delta,
}

impl<'st, Sch: SchemaInfo> Transaction<'st, Sch>
where
    Sch::Delta: Default,
{
    fn start(store: &'st mut Sch::Store) -> Self {
        Self {
            store,
            delta: Default::default(),
        }
    }
}

#[derive(Debug)]
struct TransactionRef<'st, 'tx, Sch: SchemaInfo> {
    store: &'st mut Sch::Store,
    delta: &'tx mut Sch::Delta,
}

impl<'st, 'tx, Sch: SchemaInfo> TransactionRef<'st, 'tx, Sch> {
    fn from_parts(store: &'st mut Sch::Store, delta: &'tx mut Sch::Delta) -> Self {
        Self { store, delta }
    }

    fn new(tx: &'tx mut Transaction<'st, Sch>) -> Self
    where
        'tx: 'st,
    {
        Self::from_parts(tx.store, &mut tx.delta)
    }
}

// Get particular map from the TransactionRef
trait MapGetter<I> {
    type Result;
    fn get_map(self) -> Self::Result;
}

impl<'st, 'tx, Col: ColumnInfo, SchRest: SchemaInfo> MapGetter<()>
    for TransactionRef<'st, 'tx, (Col, SchRest)>
{
    type Result = MapHandle<'st, 'tx, Col>;
    fn get_map(self) -> Self::Result {
        MapHandle::new(&mut self.store.0, &mut self.delta.0)
    }
}

impl<'st, 'tx, Col: ColumnInfo, SchRest: SchemaInfo, I> MapGetter<(I,)>
    for TransactionRef<'st, 'tx, (Col, SchRest)>
where
    TransactionRef<'st, 'tx, SchRest>: MapGetter<I>,
{
    type Result = <TransactionRef<'st, 'tx, SchRest> as MapGetter<I>>::Result;
    fn get_map(self) -> Self::Result {
        TransactionRef::from_parts(&mut self.store.1, &mut self.delta.1).get_map()
    }
}

struct MapHandle<'st, 'tx, Col: ColumnInfo> {
    store: &'st mut Col::StoreMap,
    delta: &'tx mut Col::DeltaMap,
}

impl<'st, 'tx, Col: ColumnInfo> MapHandle<'st, 'tx, Col> {
    fn new(store: &'st mut Col::StoreMap, delta: &'tx mut Col::DeltaMap) -> Self {
        Self { store, delta }
    }
}

// TODO MapHandle::{get, put, del, commit}
