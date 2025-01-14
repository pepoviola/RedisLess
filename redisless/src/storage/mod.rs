#[cfg(test)]
mod tests;

pub mod in_memory;
pub mod models;

use models::expiry::Expiry;

pub trait Storage {
    fn write(&mut self, key: &[u8], value: &[u8]);
    fn expire(&mut self, key: &[u8], expiry: Expiry) -> u32;
    fn read(&mut self, key: &[u8]) -> Option<&[u8]>;
    fn remove(&mut self, key: &[u8]) -> u32;
    fn contains(&mut self, key: &[u8]) -> bool;
}
