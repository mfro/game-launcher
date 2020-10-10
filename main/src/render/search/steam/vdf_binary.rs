use std::{fmt::Debug, fmt::Display};

use serde::{de::MapAccess, de::SeqAccess, de::Visitor, forward_to_deserialize_any, Deserializer};

use flat::prelude::*;

#[derive(Debug)]
pub struct ValveError(String);

impl Display for ValveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for ValveError {}

impl serde::de::Error for ValveError {
    fn custom<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        ValveError(msg.to_string())
    }
}

pub struct ValveDeserializer<'a, 'de> {
    src: ValveReader<'a, 'de>,
    key: Option<&'de str>,
    value: Option<ValveToken<'de>>,
}

impl<'a, 'de> ValveDeserializer<'a, 'de> {
    pub fn new(src: &'a mut &'de [u8]) -> ValveDeserializer<'a, 'de> {
        let src = ValveReader::new(src);
        let key = None;
        let value = None;
        ValveDeserializer { src, key, value }
    }
}

impl<'de> SeqAccess<'de> for ValveDeserializer<'_, 'de> {
    type Error = ValveError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: serde::de::DeserializeSeed<'de>,
    {
        let x = self.src.next();
        match x {
            None => Ok(None),
            Some(None) => Ok(None),
            Some(Some((_, value))) => {
                self.value = Some(value);
                seed.deserialize(self).map(Some)
            }
        }
    }
}

impl<'de> MapAccess<'de> for ValveDeserializer<'_, 'de> {
    type Error = ValveError;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: serde::de::DeserializeSeed<'de>,
    {
        let x = self.src.next();
        match x {
            None => Ok(None),
            Some(None) => Ok(None),
            Some(Some((key, value))) => {
                self.key = Some(key);
                self.value = Some(value);
                seed.deserialize(self).map(Some)
            }
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::DeserializeSeed<'de>,
    {
        seed.deserialize(self)
    }
}

impl<'de> Deserializer<'de> for &mut ValveDeserializer<'_, 'de> {
    type Error = ValveError;

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_seq(self)
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_map(self)
    }

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        if let Some(key) = self.key.take() {
            visitor.visit_str(key)
        } else if let Some(value) = self.value.take() {
            match value {
                ValveToken::Object => visitor.visit_map(self),
                ValveToken::String(s) => visitor.visit_str(s),
                ValveToken::I32(v) => visitor.visit_i32(v),
                ValveToken::I64(v) => visitor.visit_i64(v),
                ValveToken::U64(v) => visitor.visit_u64(v),
                ValveToken::F32(v) => visitor.visit_f32(v),
            }
        } else {
            panic!()
        }
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_some(self)
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf /* option */ unit unit_struct newtype_struct /* seq */ tuple
        tuple_struct map /* struct */ enum identifier ignored_any
    }
}

#[derive(Debug, Copy, Clone)]
pub enum ValveToken<'a> {
    Object,
    String(&'a str),
    I32(i32),
    I64(i64),
    U64(u64),
    F32(f32),
}

pub struct ValveReader<'a, 'b> {
    src: &'a mut &'b [u8],
    depth: usize,
}

impl<'a, 'b> ValveReader<'a, 'b> {
    pub fn new(src: &'a mut &'b [u8]) -> ValveReader<'a, 'b> {
        let depth = 0;
        ValveReader { src, depth }
    }

    fn read_string(&mut self) -> &'b str {
        let strlen = self.src.iter().position(|&c| c == 0).unwrap();
        let bytes = self.src.load_slice(strlen);
        *self.src = &self.src[1..];
        std::str::from_utf8(bytes).unwrap()
    }
}

impl<'a, 'b> Iterator for ValveReader<'a, 'b> {
    type Item = Option<(&'b str, ValveToken<'b>)>;

    fn next(&mut self) -> Option<Self::Item> {
        let ty: u8 = self.src.load();
        if ty == 0x08 || ty == 0x0b {
            if self.depth == 0 {
                return None;
            } else {
                self.depth -= 1;
                return Some(None);
            }
        }

        let name = self.read_string();
        let value = match ty {
            // child object
            0x00 => {
                self.depth += 1;
                ValveToken::Object
            }
            // string
            0x01 => ValveToken::String(self.read_string()),
            // int 32
            0x02 => ValveToken::I32(self.src.load()),
            // float 32
            0x03 => ValveToken::F32(self.src.load()),
            // pointer?
            0x04 => panic!("unsupported valve type {:x}", ty),
            // wide string
            0x05 => panic!("unsupported valve type {:x}", ty),
            // color?
            0x06 => panic!("unsupported valve type {:x}", ty),
            // uint 64
            0x07 => ValveToken::U64(self.src.load()),
            // probably binary?
            0x09 => panic!("unsupported valve type {:x}", ty),
            // int 64
            0xa => ValveToken::I64(self.src.load()),
            _ => panic!("unsupported valve type {:x}", ty),
        };
        Some(Some((name, value)))
    }
}
