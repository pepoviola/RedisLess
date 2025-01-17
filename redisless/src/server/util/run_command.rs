use std::sync::{Arc, Mutex};

use crate::{command::Command, storage::Storage};

use super::*;

pub fn run_command_and_get_response<T: Storage>(
    storage: &Arc<Mutex<T>>,
    bytes: &[u8; 512],
) -> (Option<Command>, CommandResponse) {
    let command = get_command(bytes);

    let response = match &command {
        Ok(command) => match command {
            Command::Set(k, v) => {
                lock_then_release(storage).write(k.as_slice(), v.as_slice());
                protocol::OK.to_vec()
            }
            Command::Setex(k, expiry, v) | Command::PSetex(k, expiry, v) => {
                let mut storage = lock_then_release(storage);

                storage.write(k.as_slice(), v.as_slice());
                storage.expire(k.as_slice(), *expiry);

                protocol::OK.to_vec()
            }
            Command::Setnx(k, v) => {
                let mut storage = lock_then_release(storage);
                match storage.contains(k) {
                    // Key exists, will not re set key
                    true => b":0\r\n".to_vec(),
                    // Key does not exist, will set key
                    false => {
                        storage.write(k, v);
                        b":1\r\n".to_vec()
                    }
                }
            }
            Command::MSet(items) => {
                let mut storage = lock_then_release(storage);
                items.iter().for_each(|(k, v)| storage.write(k, v));
                protocol::OK.to_vec()
            }
            Command::MSetnx(items) => {
                // Either set all or not set any at all if any already exist
                let mut storage = lock_then_release(storage);
                match items.iter().all(|(key, _)| !storage.contains(key)) {
                    // None of the keys already exist in the storage
                    true => {
                        items.iter().for_each(|(k, v)| storage.write(k, v));
                        b":1\r\n".to_vec()
                    }
                    // Some key exists, don't write any of the keys
                    false => b":0\r\n".to_vec(),
                }
            }
            Command::Expire(k, expiry) | Command::PExpire(k, expiry) => {
                let v = lock_then_release(storage).expire(k.as_slice(), *expiry);
                format!(":{}\r\n", v).as_bytes().to_vec()
            }
            Command::Get(k) => match lock_then_release(storage).read(k.as_slice()) {
                Some(value) => {
                    let res = format!("+{}\r\n", std::str::from_utf8(value).unwrap());
                    res.as_bytes().to_vec()
                }
                None => protocol::NIL.to_vec(),
            },
            Command::GetSet(k, v) => {
                let mut storage = lock_then_release(storage);

                let response = match storage.read(k.as_slice()) {
                    Some(value) => {
                        let res = format!("+{}\r\n", std::str::from_utf8(value).unwrap());
                        res.as_bytes().to_vec()
                    }
                    None => protocol::NIL.to_vec(),
                };
                storage.write(k.as_slice(), v.as_slice());
                response
            }
            Command::MGet(keys) => {
                // Draft, slow ?
                // better to add a response formatter module?
                let mut storage = lock_then_release(storage);
                let mut final_response = format!("*{}\r\n", keys.len());

                for key in keys {
                    let response_line = match storage.read(key.as_slice()) {
                        Some(value) => {
                            format!("+{}\r\n", std::str::from_utf8(value).unwrap())
                        }
                        None => "$-1\r\n".to_string(),
                    };
                    final_response.push_str(response_line.as_str());
                }
                final_response.as_bytes().to_vec()
            }
            Command::Del(k) => {
                let total_del = lock_then_release(storage).remove(k.as_slice());
                format!(":{}\r\n", total_del).as_bytes().to_vec()
            }
            Command::Incr(k) => {
                let mut storage = lock_then_release(storage);

                match storage.read(k.as_slice()) {
                    Some(value) => {
                        if let Ok(mut int_val) = std::str::from_utf8(value).unwrap().parse::<i64>()
                        {
                            int_val += 1;
                            let new_value = int_val.to_string().into_bytes();
                            storage.write(k.as_slice(), new_value.as_slice());

                            format!(":{}\r\n", int_val).as_bytes().to_vec()
                        } else {
                            b"-WRONGTYPE Operation against a key holding the wrong kind of value}}"
                                .to_vec()
                        }
                    }
                    None => {
                        let val = "1";
                        storage.write(k, val.as_bytes());
                        format!(":{}\r\n", val).as_bytes().to_vec()
                    }
                }
            }
            Command::Exists(k) => {
                let exists = lock_then_release(storage).contains(k);
                let exists: u32 = match exists {
                    true => 1,
                    false => 0,
                };
                format!(":{}\r\n", exists).as_bytes().to_vec()
            }
            Command::Info => protocol::EMPTY_LIST.to_vec(), // TODO change with some real info?
            Command::Ping => protocol::PONG.to_vec(),
            Command::Quit => protocol::OK.to_vec(),
        },
        Err(err) => format!("-ERR {}\r\n", err).as_bytes().to_vec(),
    };

    (command.ok(), response)
}
