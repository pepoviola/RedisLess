#[cfg(test)]
mod tests;

pub mod command_error;
mod util;

use crate::protocol::Resp;
use crate::storage::models::Expiry;
use command_error::RedisCommandError;

type Key = Vec<u8>;
type Value = Vec<u8>;
type Items = Vec<(Key, Value)>;
type Keys = Vec<Key>;

#[derive(Debug, PartialEq)]
pub enum Command {
    Set(Key, Value),
    Setnx(Key, Value),
    Setex(Key, Expiry, Value),
    PSetex(Key, Expiry, Value),
    MSet(Items),
    MSetnx(Items),
    Expire(Key, Expiry),
    PExpire(Key, Expiry),
    Get(Key),
    GetSet(Key, Value),
    MGet(Keys),
    Del(Key),
    Incr(Key),
    Exists(Key),
    Info,
    Ping,
    Quit,
}

impl Command {
    pub fn parse(v: Vec<Resp>) -> Result<Self, RedisCommandError> {
        use util::*;
        use Command::*;
        use RedisCommandError::*;

        match v.first() {
            Some(Resp::BulkString(command)) => match *command {
                b"SET" | b"set" | b"Set" => {
                    let key = get_bytes_vec(v.get(1))?;
                    let value = get_bytes_vec(v.get(2))?;

                    Ok(Set(key, value))
                }
                b"SETEX" | b"setex" | b"SetEx" | b"Setex" => {
                    let key = get_bytes_vec(v.get(1))?;
                    let duration = get_bytes_vec(v.get(2)).and_then(parse_duration)?;
                    let value = get_bytes_vec(v.get(3))?;
                    let expiry = Expiry::new_from_secs(duration)?;

                    Ok(Setex(key, expiry, value))
                }
                b"PSETEX" | b"psetex" | b"PSetEx" | b"PSetex" => {
                    let key = get_bytes_vec(v.get(1))?;
                    let duration = get_bytes_vec(v.get(2)).and_then(parse_duration)?;
                    let value = get_bytes_vec(v.get(3))?;
                    let expiry = Expiry::new_from_millis(duration)?;

                    Ok(PSetex(key, expiry, value))
                }
                b"MSET" | b"MSet" | b"mset" => {
                    // Will not panic with out of bounds, because request has at least length 1,
                    // in which case request will be an empty slice
                    // &[key, value, key, value, key, value, ...] should be even in length
                    // We want [(key, value), (key, value), (key, value), ..]
                    let pairs = &v[1..];
                    let chunk_size = 2_usize;
                    if pairs.is_empty() || pairs.len() % chunk_size != 0 {
                        return Err(ArgNumber);
                    }

                    let mut items = Vec::<(Key, Value)>::with_capacity(pairs.len());
                    for pair in pairs.chunks_exact(chunk_size) {
                        match pair {
                            [key, value] => {
                                let key = get_bytes_vec(Some(&key))?;
                                let value = get_bytes_vec(Some(&value))?;
                                items.push((key, value));
                            }
                            _ => unreachable!(), // pairs has even length so each chunk will have len 2
                        }
                    }
                    Ok(MSet(items))
                }
                b"MSETNX" | b"MSetnx" | b"msetnx" => {
                    let pairs = &v[1..];

                    let chunk_size = 2_usize;
                    if pairs.is_empty() || pairs.len() % chunk_size != 0 {
                        return Err(ArgNumber);
                    }

                    let mut items = Items::with_capacity(pairs.len());
                    for pair in pairs.chunks_exact(chunk_size) {
                        match pair {
                            [key, value] => {
                                let key = get_bytes_vec(Some(&key))?;
                                let value = get_bytes_vec(Some(&value))?;
                                items.push((key, value));
                            }
                            _ => unreachable!(),
                        }
                    }

                    Ok(MSetnx(items))
                }
                b"SETNX" | b"setnx" | b"Setnx" => {
                    let key = get_bytes_vec(v.get(1))?;
                    let value = get_bytes_vec(v.get(2))?;

                    Ok(Setnx(key, value))
                }
                b"EXPIRE" | b"expire" | b"Expire" => {
                    let key = get_bytes_vec(v.get(1))?;
                    let duration = get_bytes_vec(v.get(2)).and_then(parse_duration)?;
                    let expiry = Expiry::new_from_secs(duration)?;

                    Ok(Expire(key, expiry))
                }
                b"PEXPIRE" | b"Pexpire" | b"PExpire" | b"pexpire" => {
                    let key = get_bytes_vec(v.get(1))?;
                    let duration = get_bytes_vec(v.get(2)).and_then(parse_duration)?;
                    let expiry = Expiry::new_from_millis(duration)?;

                    Ok(PExpire(key, expiry))
                }
                b"GET" | b"get" | b"Get" => {
                    let key = get_bytes_vec(v.get(1))?;
                    Ok(Get(key))
                }
                b"GETSET" | b"getset" | b"Getset" | b"GetSet" => {
                    let key = get_bytes_vec(v.get(1))?;
                    let value = get_bytes_vec(v.get(2))?;

                    Ok(GetSet(key, value))
                }
                b"MGET" | b"mget" | b"MGet" => {
                    let keys = &v[1..]; // will never panic
                    if keys.is_empty() {
                        return Err(ArgNumber);
                    }

                    let mut keys_vec = Keys::with_capacity(keys.len());
                    for key in keys {
                        let key = get_bytes_vec(Some(key))?;
                        keys_vec.push(key);
                    }

                    Ok(MGet(keys_vec))
                }
                b"DEL" | b"del" | b"Del" => {
                    let key = get_bytes_vec(v.get(1))?;
                    Ok(Del(key))
                }
                b"INCR" | b"incr" | b"Incr" => {
                    let key = get_bytes_vec(v.get(1))?;
                    Ok(Incr(key))
                }
                b"EXISTS" | b"exists" | b"Exists" => {
                    let key = get_bytes_vec(v.get(1))?;
                    Ok(Exists(key))
                }
                b"INFO" | b"info" | b"Info" => Ok(Info),
                b"PING" | b"ping" | b"Ping" => Ok(Ping),
                b"QUIT" | b"quit" | b"Quit" => Ok(Quit),
                unsupported_command => Err(NotSupported(
                    std::str::from_utf8(unsupported_command)
                        .unwrap()
                        .to_string(),
                )),
            },
            _ => Err(InvalidCommand),
        }
    }
}
