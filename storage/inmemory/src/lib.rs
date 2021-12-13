//use storage::schema::{self, Column, HasColumn, Schema};
//use storage::traits::{self, Data, TxImpl};

//use std::collections::BTreeMap;

mod backend;

/*
/// Type tag identifying the in-memory storage backend.
struct Backend;

//impl MapForMultiplicity for Backend

type DataMapSingle = BTreeMap<Data, Data>;
type ChangeMapSingle = BTreeMap<Data, Option<Data>>;

impl<'st, Sch: SchemaImpl> traits::Transaction<'st, Sch> for Backend
    where Sch::StoreMaps: 'static
{
    type Impl = (&'st mut Sch::StoreMaps, Sch::ChangeMaps);

    /*
    fn get<'tx, Col: Column>(this: &'tx TxImpl<'st, Sch, Self>) -> MapFor<'tx, Col, Self>
        where Self: MapForMultiplicity<'tx, Col::Kind>
    {
        todo!()
    }
    */
    fn get_single<
        'tx, I
    >(this: &'tx mut TxImpl<'st, Sch, Self>) -> <Self as traits::SingleMap<'tx>>::Impl
    where
        Self: traits::SingleMap<'tx>,
        Self: traits::GetMap<'st, 'tx, Sch, I, Impl = (&'st mut Sch::StoreMaps, &'tx mut Sch::ChangeMaps)>,
        'st: 'tx,
    {
        <Self as traits::GetMap<'st, 'tx, Sch, I>>::get_single((this.0, &mut this.1))
    }
}

//trait GetMap<>

impl<
    'st: 'tx,
    'tx,
    I,
    Col: Column,
    Rest: SchemaImpl
> traits::GetMap<'st, 'tx, (Col, Rest), (I,)> for Backend
    where
        Col::Kind: MapKindImpl,
        Self: traits::GetMap<'st, 'tx, Rest, I>,
        Self: traits::Transaction<'st, Rest, Impl = (&'st mut Rest::StoreMaps, Rest::ChangeMaps)>
{
    fn get_single(
        tx: &'tx mut TxImpl<'st, (Col, Rest), Self>
    ) -> <Self as traits::SingleMap<'tx>>::Impl {
        let next = (&mut tx.0.1, &mut tx.1.1);
        <Self as traits::GetMap<'st, 'tx, Rest, I>>::get_single(&mut next)
    }
}

impl<
    'st: 'tx,
    'tx,
    Col: Column<Kind = map_kind::Single>,
    Rest: SchemaImpl,
> traits::GetMap<'st, 'tx, (Col, Rest), ()> for Backend
    where Rest::ChangeMaps: 'tx
{
    type Impl = (
        &'st mut <(Col, Rest) as SchemaImpl>::StoreMaps,
        &'tx mut <(Col, Rest) as SchemaImpl>::ChangeMaps,
    );

    fn get_single(
        txref: <Self as traits::GetMap<'st, 'tx, (Col, Rest), ()>>::Impl
    ) -> <Self as traits::SingleMap<'tx>>::Impl {
        new_map(&txref.0.0, &mut txref.1.0)
    }
}

trait MapKindImpl {
    type StoreMap: 'static;
    type ChangeMap;
}

impl MapKindImpl for map_kind::Single {
    type StoreMap = DataMapSingle;
    type ChangeMap = ChangeMapSingle;
}

trait SchemaImpl: Schema {
    type StoreMaps: 'static;
    type ChangeMaps;
}

impl SchemaImpl for () {
    type StoreMaps = ();
    type ChangeMaps = ();
}

impl<Col: Column, Rest: SchemaImpl> SchemaImpl for (Col, Rest)
    where Col::Kind: MapKindImpl
{
    type StoreMaps = (<Col::Kind as MapKindImpl>::StoreMap, Rest::StoreMaps);
    type ChangeMaps = (<Col::Kind as MapKindImpl>::ChangeMap, Rest::ChangeMaps);
}

impl<'tx> traits::SingleMap<'tx> for Backend {
    type Impl = (&'tx DataMapSingle, &'tx mut ChangeMapSingle);

    fn exists(this: &Self::Impl, key: &[u8]) -> storage::Result<bool> {
        Self::get(this, key).map(|r| r.is_some())
    }

    fn get<'a, 'b>(this: &'a Self::Impl, key: &'b [u8]) -> storage::Result<Option<&'a [u8]>> {
        let res = match this.1.get(key) {
            Some(Some(x)) => Some(x.as_ref()),
            Some(None) => None,
            None => this.0.get(key).map(AsRef::as_ref),
        };
        Ok(res)
    }

    fn put(this: &mut Self::Impl, key: &[u8], val: &[u8]) -> storage::Result<()> {
        this.1.insert(key.to_vec(), Some(val.to_vec()));
        Ok(())
    }

    fn remove(this: &mut Self::Impl, key: &[u8]) -> storage::Result<()> {
        this.1.insert(key.to_vec(), None);
        Ok(())
    }
}

fn new_map<'tx>(
    a: &'tx DataMapSingle,
    b: &'tx mut ChangeMapSingle,
) -> <Backend as traits::SingleMap<'tx>>::Impl {
    (a, b)
}

/// TODO create a new storage
pub fn new() {}

#[cfg(test)]
mod test {
}
*/
