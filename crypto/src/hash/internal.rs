use digest::{generic_array::GenericArray, Digest};

pub fn hash<D: Digest, T: AsRef<[u8]>>(in_bytes: T) -> GenericArray<u8, <D as Digest>::OutputSize> {
    let mut hasher = D::new();
    hasher.update(in_bytes);
    hasher.finalize()
}
