use std::collections::HashMap;

use nom::{
    branch::alt, bytes::complete::escaped, bytes::complete::take_while, character::complete::char,
    character::complete::none_of, character::complete::one_of, combinator::map,
    multi::separated_list, sequence::delimited, sequence::separated_pair, IResult,
};

#[derive(Debug, Clone)]
pub enum AnyValue {
    String(String),
    Map(HashMap<String, AnyValue>),
}

fn space(input: &str) -> IResult<&str, &str> {
    take_while(char::is_whitespace)(input)
}

fn string_value(input: &str) -> IResult<&str, String> {
    if input.len() >= 2 && &input[0..2] == "\"\"" {
        Ok((&input[2..], String::new()))
    } else {
        let (input, value) = delimited(
            char('"'),
            escaped(none_of("\"\\"), '\\', one_of("\"\\")),
            char('"'),
        )(input)?;

        Ok((input, value.replace("\\\"", "\"").replace("\\\\", "\\")))
    }
}

fn map_value(input: &str) -> IResult<&str, HashMap<String, AnyValue>> {
    let (input, vec) = delimited(
        char('{'),
        delimited(space, separated_list(space, key_value), space),
        char('}'),
    )(input)?;

    let map = vec.into_iter().collect();

    Ok((input, map))
}

pub fn key_value(input: &str) -> IResult<&str, (String, AnyValue)> {
    separated_pair(string_value, space, any_value)(input)
}

pub fn any_value(input: &str) -> IResult<&str, AnyValue> {
    alt((
        map(string_value, |s| AnyValue::String(s)),
        map(map_value, |m| AnyValue::Map(m)),
    ))(input)
}
