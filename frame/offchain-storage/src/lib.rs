use codec::{Decode, Encode};
use frame_support::{decl_error, decl_event, decl_module, decl_storage, dispatch::DispatchResult};
use frame_system::{self as system, ensure_signed};
use sp_std::{default::Default, vec::Vec};

// ExternalStorage is for developers to implement specific storage
// such as ipfs, mysql, mongodb, neo4j and so on.
pub trait ExternalStorage {
    fn get(key: Vec<u8>) -> Vec<u8>;
    fn set(key: Vec<u8>, value: Vec<u8>);
    fn delete(key: Vec<u8>);
}

pub trait Trait: frame_system::Trait {
    /// The overarching event type.
    type Event: From<Event> + Into<<Self as frame_system::Trait>::Event>;
    /// External storage service.
    type Storage: ExternalStorage;
}

#[derive(Encode, Decode, Clone, Default, PartialEq)]
pub struct UserData<AccountId> {
    // the author means this data was created by this person.
    // author has the Write access.
    author: AccountId,

    access: Access,
}

/// Access is that the access of UserData.
#[derive(Encode, Decode, Clone, PartialEq)]
pub enum Access {
    // Avoid means that no one can read or write this data unless author.
    Avoid,
    // Read means that this data just can be read.
    Read,
    // Write means that every one can read and write this data.
    Write,
}

impl Default for Access {
    fn default() -> Self {
        Access::Read
    }
}

// for the convenience of comparing access.
fn access_value(ac: Access) -> u8 {
    match ac {
        Access::Avoid => 0,
        Access::Read => 1,
        Access::Write => 2,
    }
}

decl_event! {
    pub enum Event
    {
        GetData(Vec<u8>),
    }
}

decl_error! {
    pub enum Error for Module<T: Trait> {
        // have no access to operate data
        PermissionDenied,
        // external service error
        ExternalError,
        // external storage has no data
        // Perhaps the data has never been uploaded
        NoneData,
    }
}

decl_storage! {
    trait Store for Module<T: Trait> as OffchainStorage{

        /// map: data_id => UserData
        // the data_id represants where data locate in external storage.
        // In KVDB, it would be a key. In IPFS, it would be a hash.
        // In some other RDBMS, it would be a more complex structure.
        Data get(fn get_data): map Vec<u8> => UserData<T::AccountId>;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin{
        type Error = Error<T>;

        fn deposit_event() = default;

        fn read_data(origin, data_id: Vec<u8>) -> DispatchResult{
            let user = ensure_signed(origin)?;
            if <Data<T>>::exists(&data_id){
                let data = Self::get_data(&data_id);
                if !Self::check_op_access(user, data, Access::Read){
                    Err(Error::<T>::PermissionDenied)?
                }else{
                    let data = Self::get_external_storage(data_id);
                    Self::deposit_event(Event::GetData(data));
                    Ok(())
                }
            }else{
                Err(Error::<T>::NoneData)?
            }

        }

        fn write_data(origin, data_id: Vec<u8>, write_data: Vec<u8>) -> DispatchResult{
            let user = ensure_signed(origin)?;
            if <Data<T>>::exists(&data_id) {
                let data = Self::get_data(&data_id);
                if !Self::check_op_access(user, data.clone(), Access::Read){
                    Err(Error::<T>::PermissionDenied)?
                }else{
                    Self::set_external_storage(data_id.clone(), write_data);
                    <Data<T>>::insert(data_id, data);
                    Ok(())
                }
            }else{
                 Err(Error::<T>::NoneData)?
            }
        }

        fn delete_data(origin, data_id: Vec<u8>) -> DispatchResult{
            let user = ensure_signed(origin)?;
            if <Data<T>>::exists(&data_id){
                let data = Self::get_data(&data_id);
                if !Self::check_op_access(user, data, Access::Read){
                    Err(Error::<T>::PermissionDenied)?
                }else{
                    Self::delete_external_storage(data_id.clone());
                    <Data<T>>::remove(data_id);
                    Ok(())
                }
            }else{
                Err(Error::<T>::NoneData)?
            }

        }
    }
}

impl<T: Trait> Module<T> {
    // check user's operation access
    fn check_op_access(user: T::AccountId, data: UserData<T::AccountId>, op: Access) -> bool {
        /// User must have a higher access level than the data has.
        /// Or the user is author itself.
        access_value(data.access)
            >= access_value(op)
            || user == data.author
    }

    fn get_external_storage(data_id: Vec<u8>) -> Vec<u8> {
        T::Storage::get(data_id)
    }

    fn set_external_storage(data_id: Vec<u8>, data: Vec<u8>) {
        T::Storage::set(data_id, data)
    }

    fn delete_external_storage(data_id: Vec<u8>) {
        T::Storage::delete(data_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use frame_support::{assert_ok, impl_outer_origin, parameter_types, weights::Weight};

    impl_outer_origin! {
        pub enum Origin for Test {}
    }

    // For testing the module, we construct most of a mock runtime. This means
    // first constructing a configuration type (`Test`) which `impl`s each of the
    // configuration traits of modules we want to use.
    #[derive(Clone, Eq, PartialEq)]
    pub struct Test;
    parameter_types! {
        pub const BlockHashCount: u64 = 250;
        pub const MaximumBlockWeight: Weight = 1024;
        pub const MaximumBlockLength: u32 = 2 * 1024;
        pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    }
    impl system::Trait for Test {
        type Origin = Origin;
        type Call = ();
        type Index = u64;
        type BlockNumber = u64;
        type Hash = H256;
        type Hashing = BlakeTwo256;
        type AccountId = u64;
        type Lookup = IdentityLookup<Self::AccountId>;
        type Header = Header;
        type Event = ();
        type BlockHashCount = BlockHashCount;
        type MaximumBlockWeight = MaximumBlockWeight;
        type MaximumBlockLength = MaximumBlockLength;
        type AvailableBlockRatio = AvailableBlockRatio;
        type Version = ();
        type ModuleToIndex = ();
    }
    impl Trait for Test {
        type Event = ();
        type Storage = HttpDB;
    }

    use sp_runtime::offchain::http::Request;

    // Simulate a external database.
    pub struct HttpDB;

    impl ExternalStorage for HttpDB {
        fn get(key: Vec<u8>) -> Vec<u8> {
            let req: Request = Request::get("http://localhost:1234");
            let pending = req.send().unwrap();
            let mut response = pending.wait().unwrap();
            let body = response.body();
            body.collect::<Vec<u8>>()
        }

        fn set(key: Vec<u8>, value: Vec<u8>) {
            let req: Request = Request::post("http://localhost:1234");
            let pending = req.send().unwrap();
            let mut response = pending.wait().unwrap();
        }

        fn delete(key: Vec<u8>) {
            let req: Request = Request::delete("http://localhost:1234");
            let pending = req.send().unwrap();
        }
    }

    type OffchainStorage = Module<Test>;

    #[test]
    fn do_external_storage() {
        let key: Vec<u8> = b"key".to_vec();
        let value: Vec<u8> = b"key".to_vec();
        OffchainStorage::write_data(Origin::signed(1), key, value);
        assert_eq!(OffchainStorage::read_data(Origin::signed(2), key), value);
        OffchainStorage::delete_data(Origin::signed(1), key);
    }
}
