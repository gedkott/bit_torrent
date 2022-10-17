use rand::{distributions::Alphanumeric, Rng};
use std::convert::TryInto;
use std::sync::mpsc::channel;
use std::thread;

pub fn read_be_u32(input: &mut &[u8]) -> Result<u32, std::array::TryFromSliceError> {
    let (int_bytes, rest) = input.split_at(std::mem::size_of::<u32>());
    *input = rest;
    int_bytes.try_into().map(u32::from_be_bytes)
}

pub fn attach_bytes(bytes: &[std::slice::Iter<'_, u8>]) -> Vec<u8> {
    bytes.iter().cloned().flatten().cloned().collect()
}

pub fn random_string() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(20)
        .map(char::from)
        .collect()
}

#[derive(Debug)]
pub enum ExecutionErr<E> {
    Err(E),
    TimedOut,
}

pub fn with_timeout<F, T, E>(f: F, duration: std::time::Duration) -> Result<T, ExecutionErr<E>>
where
    T: Send + 'static,
    E: Sync + Send + 'static,
    F: FnOnce() -> Result<T, E>,
    F: Send + 'static,
{
    let (sender, receiver) = channel();

    let work = move || {
        let r = match f() {
            Ok(t) => Ok(t),
            Err(e) => Err(ExecutionErr::Err(e)),
        };
        let _ = sender.send(r);
    };

    thread::spawn(work);

    match receiver
        .recv_timeout(duration)
        .map_err(|_timeout_err| ExecutionErr::TimedOut)
    {
        Ok(r) => r,
        Err(e) => Err(e),
    }
}
