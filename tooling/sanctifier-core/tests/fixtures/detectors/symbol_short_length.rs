use soroban_sdk::{symbol_short, Symbol};

const SHORT: Symbol = symbol_short!("OK");
const LIMIT: Symbol = symbol_short!("123456789");
const TOO_LONG: Symbol = symbol_short!("TOO_LONG_KEY");

impl Contract {
    pub fn key() -> Symbol {
        symbol_short!("POSITION_ID")
    }
}
