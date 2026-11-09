//! Legacy formatter executable; uses exactly the gdguard command implementation.
#[path = "main.rs"]
mod command;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    command::run()
}
