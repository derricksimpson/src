mod lang;

use crate::lang::helper;

fn main() {
    println!("Hello, world!");
    helper();
}

pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub struct Config {
    pub name: String,
    pub port: u16,
}

pub enum Status {
    Active,
    Inactive,
}

pub trait Service {
    fn start(&self);
}

const MAX_RETRIES: usize = 3;
